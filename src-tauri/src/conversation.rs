//! A bounded projection of explicitly supplied conversation data, never a crawler.
use crate::office::{CoveragePart, CoverageScope, CoverageStatus, OfficeProjection};
use crate::semantic::{EvidenceDraft, KnowledgeDraft};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
};

const MAX_EXPORT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_MESSAGES: usize = 1000;
const LOCATOR_FIELD: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub fn is_conversation(format: &str) -> bool {
    matches!(format, "json" | "jsonl")
}

fn field(value: &str) -> String {
    utf8_percent_encode(value, LOCATOR_FIELD).to_string()
}
fn line(value: &str) -> String {
    value.replace('\r', "\\r").replace('\n', "\\n")
}

fn linked_urls(content: &str) -> Vec<&str> {
    let pattern =
        regex::Regex::new(r#"(?i)https?://[^\s<>\"'`]+"#).expect("fixed conversation URL pattern");
    pattern
        .find_iter(content)
        .filter_map(|matched| {
            let explicit_delimiters = content[..matched.start()].ends_with('<')
                && content[matched.end()..].starts_with('>');
            let mut candidate = if explicit_delimiters {
                matched.as_str()
            } else {
                matched.as_str().trim_end_matches(['.', ',', ';'])
            };
            loop {
                if explicit_delimiters {
                    break;
                }
                let pair = match candidate.chars().last() {
                    Some(')') => Some(('(', ')')),
                    Some(']') => Some(('[', ']')),
                    Some('}') => Some(('{', '}')),
                    _ => None,
                };
                let Some((open, close)) = pair else { break };
                if candidate.matches(close).count() <= candidate.matches(open).count() {
                    break;
                }
                candidate = &candidate[..candidate.len() - 1];
            }
            reqwest::Url::parse(candidate)
                .ok()
                .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
                .map(|_| candidate)
        })
        .collect()
}

pub(crate) struct Passage {
    pub session: String,
    pub message: String,
    pub role: String,
    pub date: String,
    pub text: String,
    pub evidence: EvidenceDraft,
}

pub(crate) fn passages(text: &str) -> Result<Vec<Passage>, String> {
    let mut start = 0;
    let mut result = Vec::new();
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches('\n');
        let end = line
            .find("] ")
            .ok_or("Conversation message context is missing.")?;
        let marker = line
            .strip_prefix('[')
            .and_then(|line| line.get(..end.saturating_sub(1)))
            .ok_or("Conversation message context is invalid.")?;
        let locator = marker
            .strip_prefix("CONVERSATION ")
            .ok_or("Conversation message context is invalid.")?;
        let field = |key: &str| -> Result<String, String> {
            let prefix = format!("{key}=");
            let values = locator
                .split("; ")
                .filter_map(|part| part.strip_prefix(&prefix))
                .collect::<Vec<_>>();
            let [value] = values.as_slice() else {
                return Err(format!(
                    "Conversation context has missing or repeated {key}."
                ));
            };
            percent_decode_str(value)
                .decode_utf8()
                .map(|value| value.into_owned())
                .map_err(|_| "Conversation context is not valid UTF-8.".into())
        };
        let role = field("role")?;
        let channel = field("channel")?;
        let date = field("date")?;
        let author = field("author")?;
        result.push(Passage {
            session: field("session")?,
            message: field("message")?,
            role,
            date,
            text: line[end+2..].to_owned(),
            evidence: EvidenceDraft {
                quote: line.to_owned(), byte_start: start, byte_end: start + line.len(),
                origin: channel,
                qualifier: Some(format!("Supplied conversation message attributed to {author}; not an independently observed external fact; missing message dates stay unknown.")),
                offset_basis: Some("extracted_conversation_projection".into()),
                source_location: Some(marker.to_owned()),
            },
        });
        start += raw.len();
    }
    Ok(result)
}

/// Native message attribution is a publication invariant, independent of a
/// provider's semantic confidence about the recorded assertion.
pub(crate) fn validate_draft(
    text: &str,
    format: &str,
    draft: &KnowledgeDraft,
) -> Result<(), String> {
    if !is_conversation(format) {
        let forged_context = draft
            .entities
            .iter()
            .map(|entity| &entity.evidence)
            .chain(draft.facts.iter().map(|fact| &fact.evidence))
            .chain(
                draft
                    .relationships
                    .iter()
                    .map(|relationship| &relationship.evidence),
            )
            .chain(draft.tags.iter().map(|tag| &tag.evidence))
            .chain(
                draft
                    .correction_candidates
                    .iter()
                    .map(|candidate| &candidate.evidence),
            )
            .chain(
                draft
                    .correction_alignments
                    .iter()
                    .map(|alignment| &alignment.candidate.evidence),
            )
            .chain(draft.source_update.iter().flat_map(|update| {
                [
                    Some(&update.evidence),
                    update.source_date_evidence.as_ref(),
                    update.source_revision_evidence.as_ref(),
                ]
                .into_iter()
                .flatten()
            }))
            .any(|evidence| {
                matches!(
                    evidence.origin.to_ascii_lowercase().as_str(),
                    "user_message"
                        | "assistant_message"
                        | "tool_output"
                        | "reasoning_summary"
                        | "system_message"
                        | "developer_message"
                        | "unknown_message"
                        | "tool_call"
                ) || evidence.offset_basis.as_deref() == Some("extracted_conversation_projection")
                    || evidence
                        .source_location
                        .as_deref()
                        .is_some_and(|location| location.starts_with("CONVERSATION "))
            });
        if forged_context || text.starts_with("[CONVERSATION ") {
            return Err("Conversation message provenance requires a retained supported conversation export; ordinary source text cannot establish acquired message roles.".into());
        }
        return Ok(());
    }
    let invalid = || {
        "Conversation knowledge lost acquired message context or promoted an attributed assertion beyond its evidence.".to_owned()
    };
    let passages = passages(text).map_err(|_| invalid())?;
    let bound = |evidence: &EvidenceDraft| -> Result<&Passage, String> {
        passages
            .iter()
            .find(|passage| {
                let acquired = &passage.evidence;
                evidence.quote == acquired.quote
                    && evidence.byte_start == acquired.byte_start
                    && evidence.byte_end == acquired.byte_end
                    && evidence.origin == acquired.origin
                    && evidence.qualifier == acquired.qualifier
                    && evidence.offset_basis == acquired.offset_basis
                    && evidence.source_location == acquired.source_location
            })
            .ok_or_else(invalid)
    };
    let message_label =
        |passage: &Passage| format!("Message {} in {}", passage.message, passage.session);
    let conversation_label = |passage: &Passage| format!("Conversation {}", passage.session);
    for entity in &draft.entities {
        let passage = bound(&entity.evidence)?;
        let correct = if entity.kind == "conversation" {
            entity.label == conversation_label(passage)
        } else {
            entity.label == message_label(passage)
                && entity.kind
                    == if passage.role == "reasoning" {
                        "reasoning_event"
                    } else {
                        "conversation_message"
                    }
        };
        if !correct {
            return Err(invalid());
        }
    }
    for fact in &draft.facts {
        let passage = bound(&fact.evidence)?;
        if fact.subject != message_label(passage)
            || fact.record_key.as_deref() != Some(passage.message.as_str())
        {
            return Err(invalid());
        }
        let correct = match fact.property.as_str() {
            "statement" => fact.value == passage.text,
            "message_date" => fact.value == passage.date,
            "message_intent" => {
                matches!(
                    fact.value.as_str(),
                    "question"
                        | "proposal"
                        | "claim"
                        | "uncertain"
                        | "tool_call"
                        | "tool_report"
                        | "recorded_decision"
                        | "reasoning_event"
                ) && (fact.value != "recorded_decision" || passage.role == "user")
                    && (fact.value != "tool_call" || passage.role == "assistant_tool_call")
                    && (fact.value != "tool_report" || passage.role == "tool")
                    && (fact.value != "reasoning_event" || passage.role == "reasoning")
            }
            _ => false,
        };
        if !correct {
            return Err(invalid());
        }
    }
    for relationship in &draft.relationships {
        let passage = bound(&relationship.evidence)?;
        if relationship.kind != "message_in_conversation"
            || relationship.from != message_label(passage)
            || relationship.to != conversation_label(passage)
            || relationship.qualifier != passage.evidence.qualifier
        {
            return Err(invalid());
        }
    }
    if !draft.tags.is_empty()
        || !draft.correction_candidates.is_empty()
        || !draft.correction_alignments.is_empty()
    {
        return Err(invalid());
    }
    if let Some(update) = &draft.source_update {
        bound(&update.evidence)?;
        // Message dates describe recorded messages, not the source export's
        // version order or independently authenticated external events.
        if update.source_date.is_some()
            || update.source_date_evidence.is_some()
            || update.source_revision.is_some()
            || update.source_revision_evidence.is_some()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

fn codex_text_parts(parts: &Value, gaps: &mut Vec<String>, row: usize) -> String {
    let Some(parts) = parts.as_array() else {
        gaps.push(format!(
            "Rollout row {row} has unsupported textual parts; original retained."
        ));
        return String::new();
    };
    parts.iter().filter_map(|part| {
        if matches!(part["type"].as_str(), Some("input_text" | "output_text" | "summary_text" | "reasoning_text" | "text")) {
            part["text"].as_str().map(str::to_owned).or_else(|| {
                gaps.push(format!("Rollout row {row} includes an unreadable text field; original retained."));
                None
            })
        } else {
            gaps.push(format!("Rollout row {row} includes an unreadable or unsupported content part; original retained."));
            None
        }
    }).collect::<Vec<_>>().join("\n")
}

/// Read only supplied serialized records. Tool arguments are retained text,
/// never executable instructions. Hidden reasoning remains opaque.
fn codex_export(bytes: &[u8]) -> Result<(Value, Vec<String>), String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Codex rollout is not readable UTF-8; original retained.")?;
    let mut session = None::<String>;
    let mut messages = Vec::new();
    let mut gaps = Vec::new();
    let mut calls = HashMap::new();
    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        if row > 4000 {
            gaps.push(
                "Only the first 4000 rollout records were examined; original retained.".into(),
            );
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            gaps.push(format!(
                "Rollout row {row} is malformed JSON; original retained."
            ));
            continue;
        };
        let payload = &record["payload"];
        match record["type"].as_str() {
            Some("session_meta") => {
                let id = payload["id"].as_str().filter(|id| !id.trim().is_empty()).ok_or("Codex session metadata lacks a thread identity.")?;
                if session.as_deref().is_some_and(|prior| prior != id) {
                    return Err("Codex export mixes thread identities; no combined chronology was invented and the original remains retained.".into());
                }
                session = Some(id.into());
                if ["cwd", "originator", "cli_version"].iter().any(|key| !payload[*key].is_string()) {
                    gaps.push(format!("Rollout row {row} has partial session metadata; supplied identity is retained without filling missing metadata."));
                }
            }
            Some("response_item") => {
                let mut role = payload["role"].as_str().unwrap_or("unknown").to_owned();
                let mut author = payload["author"].as_str().unwrap_or("unknown").to_owned();
                let mut id = payload["id"].as_str().filter(|id| !id.trim().is_empty()).map(str::to_owned);
                let content = match payload["type"].as_str() {
                    Some("message") => codex_text_parts(&payload["content"], &mut gaps, row),
                    Some("function_call") => {
                        role = "assistant_tool_call".into();
                        author = payload["name"].as_str().unwrap_or("unknown").into();
                        if let Some(call) = payload["call_id"].as_str().filter(|call| !call.is_empty()) {
                            calls.insert(call.to_owned(), author.clone());
                            if id.is_none() { id = Some(format!("call-{call}")); gaps.push(format!("Rollout row {row} lacks a response ID; its locator is derived from supplied call_id and call role.")); }
                        }
                        match payload["arguments"].as_str() {
                            Some(arguments) => format!("Tool call {author} arguments: {arguments}"),
                            None => { gaps.push(format!("Rollout row {row} has unreadable tool arguments; original retained.")); String::new() }
                        }
                    }
                    Some("function_call_output") => {
                        role = "tool".into();
                        if let Some(call) = payload["call_id"].as_str().filter(|call| !call.is_empty()) {
                            author = payload["name"].as_str().map(str::to_owned).or_else(|| calls.get(call).cloned()).unwrap_or_else(|| "unknown".into());
                            if id.is_none() { id = Some(format!("output-{call}")); gaps.push(format!("Rollout row {row} lacks a response ID; its locator is derived from supplied call_id and output role.")); }
                        }
                        payload["output"].as_str().map(str::to_owned).unwrap_or_else(|| codex_text_parts(&payload["output"], &mut gaps, row))
                    }
                    Some("reasoning") => {
                        role = "reasoning".into();
                        if payload["encrypted_content"].as_str().is_some_and(|value| !value.is_empty()) {
                            gaps.push(format!("Rollout row {row} contains encrypted reasoning that is unreadable; only supplied readable summary/content can support knowledge."));
                        }
                        let summary = codex_text_parts(&payload["summary"], &mut gaps, row);
                        if payload["content"].is_array() {
                            gaps.push(format!("Rollout row {row} contains additional reasoning content retained in the original; only summary text is projected by this adapter."));
                        }
                        summary
                    }
                    _ => { gaps.push(format!("Rollout row {row} has an unsupported response item; original retained.")); continue; }
                };
                if content.is_empty() { gaps.push(format!("Rollout row {row} has no readable message text; original retained.")); continue; }
                if id.is_none() { gaps.push(format!("Rollout row {row} lacks an original response identity; export-row locator only.")); }
                messages.push(json!({"id":id.unwrap_or_else(|| format!("rollout-row-{row}")),"role":role,"author":author,"date":record["timestamp"],"content":content}));
            }
            Some("event_msg" | "turn_context" | "token_usage_record") => {}
            _ => gaps.push(format!("Rollout row {row} is outside this response adapter; its complete record remains in the original.")),
        }
    }
    let session = session
        .ok_or("Codex rollout lacks supplied session metadata; no thread identity was invented.")?;
    Ok((
        json!({"format":"knowledge-garden-conversation-v1","conversation_id":session,"messages":messages}),
        gaps,
    ))
}

pub fn extract(path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_EXPORT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_EXPORT_BYTES {
        return Err("Conversation export exceeds the 8 MiB projection limit; the exact original is retained.".into());
    }
    let (export, mut gaps) = if format == "jsonl" {
        codex_export(&bytes)?
    } else {
        (
            serde_json::from_slice::<Value>(&bytes)
                .map_err(|_| "Conversation export is not readable JSON.".to_owned())?,
            Vec::new(),
        )
    };
    if export["format"] != "knowledge-garden-conversation-v1" {
        return Err("This JSON is not a supported conversation export adapter.".into());
    }
    let session = export["conversation_id"]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .ok_or("Conversation export has no conversation identity.")?;
    let messages = export["messages"]
        .as_array()
        .ok_or("Conversation export has no message array.")?;
    let mut ids = HashSet::new();
    let mut rows = Vec::new();
    let mut references = Vec::new();
    for (index, message) in messages.iter().take(MAX_MESSAGES).enumerate() {
        let fallback = format!("entry-{}", index + 1);
        let supplied_id = message["id"].as_str().filter(|v| !v.trim().is_empty());
        let id = supplied_id.unwrap_or(&fallback);
        if !ids.insert(id.to_owned()) {
            gaps.push(format!(
                "Message {} has a duplicate identity; its text remains in the original.",
                index + 1
            ));
            continue;
        }
        if supplied_id.is_none() {
            gaps.push(format!(
                "Message {} lacks its original identity; entry order is a projection locator only.",
                index + 1
            ));
        }
        let Some(content) = message["content"].as_str() else {
            gaps.push(format!(
                "Message {} has unsupported or missing textual content.",
                index + 1
            ));
            continue;
        };
        let role = match message["role"].as_str() {
            Some(
                role @ ("user"
                | "assistant"
                | "assistant_tool_call"
                | "tool"
                | "reasoning"
                | "system"
                | "developer"),
            ) => role,
            _ => {
                gaps.push(format!("Message {} has a missing or unsupported role; its attribution remains unknown.", index + 1));
                "unknown"
            }
        };
        let channel = match role {
            "user" => "user_message",
            "assistant" => "assistant_message",
            "assistant_tool_call" => "tool_call",
            "tool" => "tool_output",
            "reasoning" => "reasoning_summary",
            "system" => "system_message",
            "developer" => "developer_message",
            _ => "unknown_message",
        };
        let author = message["author"].as_str().unwrap_or("unknown");
        let date = match message["date"].as_str().filter(|value| !value.is_empty()) {
            Some("unknown") | None => "unknown",
            Some(value) if chrono::DateTime::parse_from_rfc3339(value).is_ok() => value,
            Some(_) => {
                gaps.push(format!("Message {} has a malformed timestamp; its date remains unknown and the supplied value remains in the original.", index + 1));
                "unknown"
            }
        };
        for url in linked_urls(content) {
            references.push(format!("- <{url}> — linked-only occurrence in `session={}; message={}; order={}; role={}; author={}; date={}; channel={}`; destination not acquired.",
                field(session), field(id), index + 1, field(role), field(author), field(date), channel));
        }
        rows.push(format!("[CONVERSATION session={}; message={}; order={}; role={}; author={}; date={}; channel={}] {} ({}; not an independently observed external fact; date is supplied message metadata or unknown, never import time)",
            field(session), field(id), index+1, field(role), field(author), field(date), channel, line(content), channel));
    }
    if messages.len() > MAX_MESSAGES {
        gaps.push(
            "Only the first 1000 messages were projected; the complete export remains retained."
                .into(),
        );
    }
    let semantic_text = rows.join("\n");
    let longest = semantic_text
        .split(|c| c != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.saturating_add(1).max(3));
    let mut markdown = format!("# {}\n\n[Open original](ORIGINAL_ASSET)\n\n## Conversation {}\n\nMessage order follows supplied export order; absent dates remain unknown. Tool output and model claims remain attributed assertions.\n\n{fence}text\n{semantic_text}\n{fence}\n", line(title), field(session));
    if !references.is_empty() {
        markdown.push_str("\n## Linked references\n\n");
        markdown.push_str(&references.join("\n"));
        markdown.push('\n');
    }
    for gap in &gaps {
        markdown.push_str(&format!("\n- Coverage gap: {gap}\n"));
    }
    let partial = !gaps.is_empty();
    Ok(OfficeProjection {
        line_count: rows.len(),
        semantic_text,
        markdown,
        partial,
        coverage: vec![CoveragePart {
            scope: CoverageScope::ConversationMessages,
            status: if partial {
                CoverageStatus::Partial
            } else {
                CoverageStatus::Complete
            },
            source_location: Some(format!("conversation {}", field(session))),
            detail: if partial {
                gaps.join(" ")
            } else {
                "Supplied message text, order, identity, role and available dates preserved; URLs are linked occurrences only.".into()
            },
        }],
        detail: if partial {
            "Conversation text has explicit coverage gaps; original retained.".into()
        } else {
            "Normalized conversation JSON projected with explicit message context and uncertain external truth.".into()
        },
    })
}
