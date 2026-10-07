//! Rust-only semantic provider transports. Request bodies and credentials never enter logs.
use crate::semantic::{
    CorrectionAlignmentDraft, CorrectionCandidateDraft, EntityDraft, EvidenceDraft, FactDraft,
    FieldAlignmentOutcome, KnowledgeDraft, ProviderError, RelationshipDraft, SemanticDecision,
    SemanticProvider, SourceUpdateDraft, SourceUpdateRole, TagDraft,
};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    process::Command,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

const OPENROUTER_SYSTEM_ONE: &str = "https://openrouter.ai/api/v1/systemone";
const JEV_MODEL: &str = "jev-1.13";
const KEYCHAIN_SERVICE: &str = "Knowledge Garden Provider Keys";
const SUPPORT_THRESHOLD: f64 = 0.6;

pub struct JevSemanticProvider {
    transport: Arc<dyn SystemOneTransport>,
    api_key: Option<String>,
    keychain_account: Option<String>,
}

pub trait SystemOneTransport: Send + Sync {
    fn complete(&self, api_key: &str, request: &Value)
        -> std::result::Result<Value, ProviderError>;
}

struct OpenRouterSystemOneTransport {
    client: Client,
}

impl Default for OpenRouterSystemOneTransport {
    fn default() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .unwrap_or_else(|_| Client::new()),
        }
    }
}

impl SystemOneTransport for OpenRouterSystemOneTransport {
    fn complete(
        &self,
        api_key: &str,
        request: &Value,
    ) -> std::result::Result<Value, ProviderError> {
        let response = self
            .client
            .post(OPENROUTER_SYSTEM_ONE)
            .bearer_auth(api_key)
            .header("Accept", "application/json")
            .header("HTTP-Referer", "https://knowledge-garden.local")
            .header("X-Title", "Knowledge Garden")
            .json(request)
            .send();
        let response = match response {
            Ok(response) if response.status().is_success() => response,
            Ok(response) => {
                let status = response.status();
                let retryable = status.as_u16() == 401
                    || status.as_u16() == 403
                    || status.as_u16() == 408
                    || status.as_u16() == 429
                    || status.as_u16() >= 500;
                let resolution = if retryable {
                    "semantic work will retry automatically"
                } else {
                    "semantic processing failed and provider configuration or implementation needs attention"
                };
                return Err(ProviderError {
                    message: format!("Jev provider returned HTTP {status}; {resolution}."),
                    retryable,
                });
            }
            Err(_) => {
                return Err(ProviderError::recoverable(
                    "Jev could not be reached; semantic work will retry automatically.".into(),
                ))
            }
        };
        response.json::<Value>().map_err(|_| {
            ProviderError::recoverable(
                "Jev returned a malformed response; semantic work will retry automatically.".into(),
            )
        })
    }
}

impl JevSemanticProvider {
    pub fn from_environment_and_keychain() -> Self {
        Self {
            transport: Arc::new(OpenRouterSystemOneTransport::default()),
            api_key: std::env::var("OPENROUTER_API_KEY")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            keychain_account: Some("openrouter".into()),
        }
    }

    pub fn with_transport(api_key: String, transport: Arc<dyn SystemOneTransport>) -> Self {
        Self {
            transport,
            api_key: Some(api_key),
            keychain_account: None,
        }
    }

    fn evaluate(
        &self,
        text: &str,
        prior_source_text: Option<&str>,
        questions: Value,
    ) -> std::result::Result<Value, ProviderError> {
        if text.len() > 64 * 1024 {
            return Err(ProviderError::recoverable(
                "This text is larger than the current Jev request limit; semantic work will retry automatically.".into(),
            ));
        }
        let key = match self.api_key.as_deref() {
            Some(key) => key.to_owned(),
            None => {
                let Some(account) = self.keychain_account.as_deref() else {
                    return Err(ProviderError::recoverable(
                        "Jev is waiting for an OpenRouter key in the development environment."
                            .into(),
                    ));
                };
                keychain_credential(account)?
            }
        };
        let mut state = serde_json::Map::from_iter([("source_text".into(), json!(text))]);
        if let Some(prior) = prior_source_text {
            state.insert("prior_source_projection".into(), json!(prior));
        }
        let request = json!({"model": JEV_MODEL, "state": state, "questions": questions});
        self.transport.complete(&key, &request)
    }
}

impl SemanticProvider for JevSemanticProvider {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        self.form_knowledge_with_prior(source_text, None)
    }

    fn form_knowledge_with_prior(
        &self,
        source_text: &str,
        prior_source_text: Option<&str>,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        let photo_mode = source_text.starts_with("[PHOTO ");
        let mut candidates = if photo_mode {
            Vec::new()
        } else {
            candidates(source_text)
        };
        let correction_candidates = correction_candidates(source_text, &candidates);
        let office_candidates = office_statement_candidates(source_text);
        let office_mode = !office_candidates.is_empty() || source_text.contains("; channel=");
        candidates.extend(office_candidates);
        candidates.sort_by_key(|candidate| (candidate.start, candidate.end));
        if candidates.is_empty() {
            return Err(ProviderError::recoverable(
                "Jev has no grounded candidate spans for this text yet; semantic coverage remains incomplete.".into(),
            ));
        }
        if candidates.len() > 64 {
            return Err(ProviderError::recoverable("This source produced more than 64 bounded semantic candidates; semantic coverage remains incomplete rather than silently truncating the source.".into()));
        }
        let update_spans = source_spans(source_text);
        if update_spans.is_empty() || update_spans.len() > 64 {
            return Err(ProviderError::recoverable("Jev source-update classification has no bounded evidence spans; semantic coverage remains incomplete.".into()));
        }
        if candidates
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Event)
            .count()
            > 1
        {
            return Err(ProviderError::recoverable("This source contains multiple event identifiers; event-to-fact grouping is not yet supported, so semantic coverage remains incomplete.".into()));
        }
        let mut questions = BTreeMap::<String, Value>::new();
        let mut update_questions = BTreeMap::<String, Value>::new();
        let event_context = candidates
            .iter()
            .find(|candidate| candidate.kind == CandidateKind::Event)
            .map(|candidate| candidate.value.as_str())
            .unwrap_or("the described event");
        for (i, candidate) in candidates.iter().enumerate() {
            if candidate.kind == CandidateKind::Event {
                questions.insert("event_identity".into(), json!({
                    "type":"choice",
                    "instructions":format!("Resolve the role of this exact candidate identifier `{}` using the whole source and the displayed source span. Short identifiers can name an observation even without the word 'Observation' immediately before them. Decide event when the source uses this ID to name an observation/event, including a person observing or recording that ID. Decide not_event only when the source identifies it as a different kind of thing (for example a revision number, a person, or a venue). Span: {}", candidate.value, candidate.quote),
                    "criteria":{"event":"This identifier names an observation or event described by the source", "not_event":"This identifier names a different kind of thing, not the described observation/event", "unclear":"The source does not resolve the identifier's role"}
                }));
                continue;
            }
            if candidate.kind == CandidateKind::Person {
                continue;
            }
            let judgment = match candidate.kind {
                CandidateKind::Event => "Does this source span explicitly identify the named event/entity ID shown in the candidate? The event ID is a source label, not a claim about its truth.",
                CandidateKind::Place => "Does this exact source span place the described event at the named place? Phrases such as 'at Riverside' or 'Location: Riverside' explicitly attach the place to the event; keep any uncertainty in the qualification.",
                CandidateKind::Count if candidate.origin == "quoted" => "Does the source explicitly quote or report the qualified count described by this candidate? Judge whether the source contains that quoted assertion, not whether the count was independently observed.",
                CandidateKind::Reference => "Does the supplied text contain this exact URL as a reference occurrence? Do not infer or fetch its destination content.",
                CandidateKind::PhotoStatement => "Does this exact native photo channel statement contain useful evidence for a photo page? Preserve the exact text and channel qualifiers. Metadata/captions are unauthenticated, OCR is transcription rather than a verified event, and classification is uncertain model interpretation. Include an explicit unknown capture date rather than inventing one. Do not infer people, dates, counts or scene objects beyond this channel statement.",
                CandidateKind::OfficeStatement => "Is this exact extracted Office passage a meaningful source statement worth representing on a source knowledge page? This includes qualified table measurements, short visible count/value fragments, and speaker-note instructions or corrections; do not drop them merely because they are provisional, conditional, or not a complete sentence. Preserve table headers, every qualification, and visible-slide versus speaker-note origin. Do not convert a proposed correction or conditional instruction into a completed event. Reject headings or labels alone. Judge what the source states, not whether it is true in the world.",
                _ => "Does this exact source span support the candidate as a statement made by the source? Do not judge whether the statement is true in the world.",
            };
            questions.insert(format!("support_{i}"), json!({
                "type":"noul",
                "instructions":format!("{judgment} Candidate value: {}; preserved qualification: {}; origin: {}. Exact source span: {}", candidate.value, candidate.qualifier.as_deref().unwrap_or("none stated"), candidate.origin, candidate.quote),
                "criteria":{"true":"The value and its stated meaning are supported by the span", "false":"The value is unsupported, overstates the span, or loses an explicit qualification"}
            }));
        }
        let update_criteria = update_spans
            .iter()
            .enumerate()
            .map(|(index, span)| {
                (
                    format!("span_{index}"),
                    json!({"quote":span.quote,"byte_start":span.start,"byte_end":span.end}),
                )
            })
            .collect::<serde_json::Map<String, Value>>();
        let update_question_set = if prior_source_text.is_some() {
            &mut update_questions
        } else {
            &mut questions
        };
        update_question_set.insert("source_update_role".into(), json!({
            "type":"choice",
            "instructions":"Classify document-level scope for this incoming source version in relation to `prior_source_projection` in state, when supplied. Compare the whole incoming source with that prior complete projection. `complete_replacement` means the incoming document is a complete snapshot of the same record and its changed values and omissions apply to this version. `targeted_correction` changes only explicitly named fact keys and does not authorize withdrawing omitted facts. `supplement` adds independent information without replacing prior support. `conditional` means the document-level update itself awaits verification or authorization. `unknown` means scope is not established. A provisional, estimated, or uncertain qualifier attached to one table row or statement qualifies that fact only; it does not by itself make the whole source update conditional. Preserve that qualifier on the fact. Do not infer source authority from a changed path, same filename, or changed value alone. If document-level scope cannot be distinguished from a fact-level qualifier, choose unknown. Choose source-update evidence only from the incoming spans below. Do not cite prior context as evidence.",
            "criteria":{"complete_replacement":"The whole source is explicitly an authoritative replacement for the earlier complete source", "supplement":"This adds independent support/context and retains earlier source content", "conditional":"This is tentative, rejected, quoted, or awaiting verification", "targeted_correction":"Only named facts are corrected; omitted facts remain active", "unknown":"The relationship or scope cannot be established from source evidence"}
        }));
        update_question_set.insert("source_update_evidence".into(), json!({
            "type":"choice",
            "instructions":"Choose the exact source span that supports your source_update_role judgment, including the words that establish negation, quotation, condition, supplement, or full replacement scope. If no sentence is decisive, choose the span that best shows why the relationship remains unknown. The span text and offsets are supplied in the criteria.",
            "criteria":update_criteria
        }));
        if prior_source_text.is_some() {
            for (index, correction) in correction_candidates.iter().enumerate() {
                update_questions.insert(format!("field_alignment_{index}"), json!({
                    "type":"choice",
                    "instructions":format!(
                        "Judge whether the incoming source explicitly corrects this exact current fact for the same event. Candidate: event `{}`, property `{}`, stated previous value `{}`, proposed current value `{}`. Exact incoming evidence span: {}. The previous current fact and its evidence are in `prior_source_projection` in state. Choose same_field_correction only when the incoming span clearly updates that same event and same property to the proposed value, and is not conditional, hypothetical, quoted as a rejected proposal, or awaiting authorization. Choose conditional_or_rejected for inactive proposals. Choose different_field_or_event for a different target. Otherwise choose uncertain. Do not infer correction from number similarity alone.",
                        correction.event_label,
                        correction.property,
                        correction.previous_value,
                        correction.corrected_value,
                        correction.evidence.quote
                    ),
                    "criteria":{
                        "same_field_correction":"The incoming span explicitly updates the same event field from its stated prior value to the proposed current value.",
                        "conditional_or_rejected":"The incoming span is hypothetical, tentative, awaiting verification or authorization, quoted as a proposal, or explicitly rejected.",
                        "different_field_or_event":"The incoming span assigns the candidate to another field or distinct event.",
                        "uncertain":"The event, field, or value relationship is not clear enough to apply."
                    }
                }));
            }
        }
        let report_dates = report_date_candidates(source_text);
        let mut report_date_criteria = serde_json::Map::new();
        for (index, candidate) in report_dates.iter().enumerate() {
            report_date_criteria.insert(
                format!("source_date_{index}"),
                json!({"value":candidate.iso,"span":candidate.evidence.quote}),
            );
        }
        report_date_criteria.insert(
            "none".into(),
            json!("No reliable source/report version date is stated."),
        );
        questions.insert("source_order_date".into(), json!({
            "type":"choice",
            "instructions":"Select a date only when the source explicitly identifies it as this document/report/version’s own date. Do not choose the observed event date, import/receipt date, a quoted/rejected date, a conditional date, or a date whose calendar value is invalid. Choose none when no reliable source date is stated.",
            "criteria":report_date_criteria
        }));
        let report_revisions = report_revision_candidates(source_text);
        let mut revision_criteria = serde_json::Map::new();
        for (index, candidate) in report_revisions.iter().enumerate() {
            revision_criteria.insert(
                format!("source_revision_{index}"),
                json!({"value":candidate.revision,"span":candidate.evidence.quote}),
            );
        }
        revision_criteria.insert(
            "none".into(),
            json!("No reliable source revision number is stated."),
        );
        questions.insert("source_order_revision".into(), json!({
            "type":"choice",
            "instructions":"Select a revision number only when the source identifies it as the active version/revision of this complete source. Do not select event IDs, quoted/rejected proposals, targeted-correction references, or conditional/obsolete revision mentions. Choose none when no reliable revision is stated.",
            "criteria":revision_criteria
        }));
        if candidates.iter().any(|candidate| {
            matches!(
                candidate.kind,
                CandidateKind::Date | CandidateKind::UnknownDate
            )
        }) {
            let date_options = candidates
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    matches!(
                        candidate.kind,
                        CandidateKind::Date | CandidateKind::UnknownDate
                    )
                })
                .map(|(i, candidate)| {
                    (
                        format!("date_{i}"),
                        json!({"value":candidate.value,"span":candidate.quote}),
                    )
                })
                .collect::<serde_json::Map<String, Value>>();
            let mut criteria = serde_json::Map::new();
            for (option, candidate) in &date_options {
                criteria.insert(option.clone(), candidate.clone());
            }
            criteria.insert(
                "none".into(),
                json!("No candidate describes an event date."),
            );
            questions.insert("event_date".into(), json!({
                "type":"choice",
                "instructions":"Which candidate date, if any, is the date of the described observation event? A report revision date is not an event date. Choose none when the source does not state an event date.",
                "criteria":criteria
            }));
        }
        let pairs = person_pairs(&candidates);
        for (i, pair) in pairs.iter().enumerate() {
            questions.insert(format!("person_identity_{i}"), json!({
                "type":"choice",
                "instructions":format!("How does the source relate the two person mentions? First: {}. Second: {}. Use the source evidence and do not equate people solely because their names overlap.", pair.0.value, pair.1.value),
                "criteria":{"distinct":"The source establishes these are distinct people", "uncertain":"The source does not establish whether these are the same person", "same":"The source explicitly establishes these are the same person"}
            }));
        }
        for (i, candidate) in candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c.kind, CandidateKind::Person | CandidateKind::Place))
        {
            questions.insert(format!("relation_{i}"), json!({
                "type":"choice",
                "instructions":format!("Classify the role, if any, of this exact entity mention in {event_context}. The full source is in state. Mention: {}. For a person, observer includes a person who reports or estimates the event's observation/count when the source attributes that report to them; it does not assert independent truth. Choose observer/attendee/location only when the source connects this mention to that specific event; otherwise choose other or none.", candidate.quote),
                "criteria": {
                    "observer":"The entity is stated as observing or reporting the event",
                    "attendee":"The entity is stated as attending the event",
                    "location":"The entity is stated as the event location",
                    "other":"The entity is mentioned but no event relation is established",
                    "none":"The candidate role is unsupported"
                }
            }));
        }
        let result = self.evaluate(
            source_text,
            None,
            Value::Object(questions.into_iter().collect()),
        )?;
        let mut result_answers = result
            .get("answers")
            .and_then(Value::as_object)
            .ok_or_else(|| ProviderError::recoverable("Jev response has no typed answers.".into()))?
            .clone();
        let mut result_model = result
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(JEV_MODEL)
            .to_owned();
        if !update_questions.is_empty() {
            // Keep source-scope reconciliation focused: a row-level qualifier must
            // not be drowned out by the unrelated extraction judgments in this batch.
            let focused = self.evaluate(
                source_text,
                prior_source_text,
                Value::Object(update_questions.into_iter().collect()),
            )?;
            let focused_answers = focused
                .get("answers")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    ProviderError::recoverable(
                        "Jev source-update response has no typed answers.".into(),
                    )
                })?;
            result_answers.extend(focused_answers.clone());
            result_model = focused
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(JEV_MODEL)
                .to_owned();
        }
        let answers = &result_answers;
        let validated = validate_answers(
            source_text,
            &candidates,
            &pairs,
            answers,
            &update_spans,
            &report_dates,
            &report_revisions,
        )?;
        let correction_alignments = correction_candidates
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                let key = format!("field_alignment_{index}");
                let (choice, certainty) = validated_choice(
                    &result_answers,
                    &key,
                    &[
                        "same_field_correction",
                        "conditional_or_rejected",
                        "different_field_or_event",
                        "uncertain",
                    ],
                )
                .ok()?;
                let outcome = match choice.as_str() {
                    "same_field_correction" => FieldAlignmentOutcome::SameFieldCorrection,
                    "conditional_or_rejected" => FieldAlignmentOutcome::ConditionalOrRejected,
                    "different_field_or_event" => FieldAlignmentOutcome::DifferentFieldOrEvent,
                    _ => FieldAlignmentOutcome::Uncertain,
                };
                Some(CorrectionAlignmentDraft {
                    candidate: candidate.clone(),
                    outcome,
                    certainty,
                    model: result_model.clone(),
                })
            })
            .collect::<Vec<_>>();
        let mut decisions = Vec::new();
        let model = result_model;
        let mut accepted = Vec::new();
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Event)
        {
            let Some((choice, confidence)) = validated.event_identity.as_ref() else {
                return Err(ProviderError::recoverable(
                    "No supported event identity was found; semantic coverage remains incomplete and will retry automatically.".into(),
                ));
            };
            decisions.push(SemanticDecision {
                question: format!("event_identity:{}", candidate.value),
                model: model.clone(),
                outcome: if choice == "event" && *confidence >= 0.55 {
                    "event"
                } else {
                    "unresolved"
                }
                .into(),
                probability: Some(*confidence),
            });
            if choice == "event" && *confidence >= 0.55 {
                accepted.push(candidate);
            }
        }
        for candidate in candidates.iter().filter(|candidate| {
            matches!(candidate.kind, CandidateKind::Person | CandidateKind::Place)
        }) {
            let index = candidates
                .iter()
                .position(|value| std::ptr::eq(value, candidate))
                .unwrap_or(0);
            let (role, probability) = validated.entity_roles.get(&index).unwrap();
            let judged_role = if candidate.kind == CandidateKind::Place
                && (role == "location" && *probability >= 0.6
                    || validated
                        .candidate_support
                        .get(&index)
                        .is_some_and(|support| *support >= SUPPORT_THRESHOLD))
            {
                "location"
            } else {
                role.as_str()
            };
            decisions.push(SemanticDecision {
                question: format!("entity_role:{}", candidate.value),
                model: model.clone(),
                outcome: judged_role.into(),
                probability: Some(*probability),
            });
            let selected = match candidate.kind {
                CandidateKind::Person => role != "none" && *probability >= 0.6,
                CandidateKind::Place => {
                    (role == "location" && *probability >= 0.6)
                        || validated
                            .candidate_support
                            .get(&index)
                            .is_some_and(|support| *support >= SUPPORT_THRESHOLD)
                }
                _ => false,
            };
            if selected {
                accepted.push(candidate);
            }
        }
        for (i, candidate) in candidates.iter().enumerate() {
            if matches!(candidate.kind, CandidateKind::Event | CandidateKind::Person) {
                continue;
            }
            let answer = *validated.candidate_support.get(&i).unwrap();
            decisions.push(SemanticDecision {
                question: format!("candidate_support:{}", candidate.quote),
                model: result
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or(JEV_MODEL)
                    .to_owned(),
                outcome: if answer >= SUPPORT_THRESHOLD {
                    "supported"
                } else {
                    "unsupported"
                }
                .into(),
                probability: Some(answer),
            });
            let is_correction = correction_candidates.iter().any(|correction| {
                correction.evidence.byte_start == candidate.start
                    && correction.evidence.byte_end == candidate.end
            });
            if answer >= SUPPORT_THRESHOLD && !is_correction {
                accepted.push(candidate);
            }
        }
        let mut accepted_indices = HashSet::new();
        accepted.retain(|candidate| {
            candidates
                .iter()
                .position(|candidate_ref| std::ptr::eq(candidate_ref, *candidate))
                .is_some_and(|index| accepted_indices.insert(index))
        });
        let event = accepted
            .iter()
            .find(|candidate| candidate.kind == CandidateKind::Event)
            .copied();
        if event.is_none()
            && !accepted
                .iter()
                .any(|candidate| candidate.kind == CandidateKind::Reference)
            && !office_mode
        {
            if let Some((choice, probability)) = validated.event_identity.as_ref() {
                return Err(ProviderError::recoverable(format!(
                    "No supported event identity was found (Jev choice {choice}, probability {probability:.2}); semantic coverage remains incomplete and will retry automatically."
                )));
            }
            return Err(ProviderError::recoverable(
                "No supported event identity or acquired reference was found; semantic coverage remains incomplete and will retry automatically.".into(),
            ));
        }
        let mut draft = KnowledgeDraft {
            decisions,
            correction_candidates,
            correction_alignments,
            ..KnowledgeDraft::default()
        };
        draft.source_update = Some(validated.source_update.clone().unwrap());
        draft.decisions.push(SemanticDecision {
            question: "source_update_role".into(),
            model: model.clone(),
            outcome: validated
                .source_update
                .as_ref()
                .unwrap()
                .role
                .as_str()
                .into(),
            probability: Some(validated.source_update.as_ref().unwrap().certainty),
        });
        if let Some((choice, probability)) = &validated.event_date {
            draft.decisions.push(SemanticDecision {
                question: "event_date".into(),
                model: model.clone(),
                outcome: choice.to_owned(),
                probability: Some(*probability),
            });
        }
        let mut merged_mentions = HashMap::<usize, usize>::new();
        if !pairs.is_empty() {
            for (i, pair) in pairs.iter().enumerate() {
                let (choice, probability) = &validated.person_identity[i];
                let first_index = candidates
                    .iter()
                    .position(|c| std::ptr::eq(c, pair.0))
                    .unwrap_or(0);
                let second_index = candidates
                    .iter()
                    .position(|c| std::ptr::eq(c, pair.1))
                    .unwrap_or(0);
                let between =
                    &source_text[pair.0.start.min(pair.1.start)..pair.0.end.max(pair.1.end)];
                let corroborated_same =
                    regex(r"(?i)\b(same person|also known as|aka|both refer to the same)\b")
                        .is_match(between);
                let merged = choice == "same" && *probability >= 0.9 && corroborated_same;
                if merged {
                    merged_mentions.insert(second_index, first_index);
                }
                draft.decisions.push(SemanticDecision {
                    question: "person_identity".into(),
                    model: model.clone(),
                    outcome: if merged {
                        "same_identity"
                    } else if choice == "distinct" {
                        "distinct_identity"
                    } else {
                        "unresolved_distinct_mentions"
                    }
                    .into(),
                    probability: Some(*probability),
                });
            }
        }
        let mut person_labels = HashMap::<usize, String>::new();
        for candidate in accepted
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Person)
        {
            let idx = candidates
                .iter()
                .position(|c| std::ptr::eq(c, *candidate))
                .unwrap_or(0);
            if let Some(canonical) = merged_mentions
                .get(&idx)
                .and_then(|index| person_labels.get(index))
                .cloned()
            {
                person_labels.insert(idx, canonical);
                continue;
            }
            let relation = &validated.entity_roles.get(&idx).unwrap().0;
            let same_name_mentions = pairs.iter().any(|(first, second)| {
                std::ptr::eq(*first, *candidate) || std::ptr::eq(*second, *candidate)
            });
            let suffix = if same_name_mentions {
                format!(
                    "; mention {}",
                    candidates
                        .iter()
                        .filter(|other| other.kind == CandidateKind::Person
                            && other.value.eq_ignore_ascii_case(&candidate.value)
                            && other.start <= candidate.start)
                        .count()
                )
            } else {
                String::new()
            };
            person_labels.insert(idx, format!("{} ({}{})", candidate.value, relation, suffix));
        }
        let mut entity_labels = HashMap::<String, EntityDraft>::new();
        if let Some(event) = event {
            let label = event.value.clone();
            entity_labels.insert(
                label.clone(),
                EntityDraft {
                    kind: "event".into(),
                    label: label.clone(),
                    evidence: event.evidence(),
                },
            );
        }
        for candidate in &accepted {
            match candidate.kind {
                CandidateKind::Person => {
                    let idx = candidates
                        .iter()
                        .position(|c| std::ptr::eq(c, *candidate))
                        .unwrap_or(0);
                    let label = person_labels.get(&idx).unwrap().clone();
                    entity_labels
                        .entry(label.clone())
                        .or_insert_with(|| EntityDraft {
                            kind: "person".into(),
                            label,
                            evidence: candidate.evidence(),
                        });
                }
                CandidateKind::Place => {
                    let idx = candidates
                        .iter()
                        .position(|c| std::ptr::eq(c, *candidate))
                        .unwrap_or(0);
                    let (role, _) = validated.entity_roles.get(&idx).unwrap();
                    if role != "location"
                        && !validated
                            .candidate_support
                            .get(&idx)
                            .is_some_and(|support| *support >= SUPPORT_THRESHOLD)
                    {
                        continue;
                    }
                    let label = candidate.value.clone();
                    entity_labels
                        .entry(label.clone())
                        .or_insert_with(|| EntityDraft {
                            kind: "place".into(),
                            label,
                            evidence: candidate.evidence(),
                        });
                }
                CandidateKind::Reference => {
                    let label = candidate.value.clone();
                    entity_labels
                        .entry(label.clone())
                        .or_insert_with(|| EntityDraft {
                            kind: "reference".into(),
                            label,
                            evidence: candidate.evidence(),
                        });
                }
                _ => {}
            }
        }
        if office_mode {
            if let Some(first) = accepted.iter().find(|candidate| {
                matches!(
                    candidate.kind,
                    CandidateKind::OfficeStatement | CandidateKind::PhotoStatement
                )
            }) {
                let channel = office_channel_at(source_text, first.start).unwrap_or_default();
                let document_label = if photo_mode {
                    "Imported photo"
                } else if matches!(
                    channel.as_str(),
                    "slide_text" | "slide_text_table" | "speaker_notes"
                ) {
                    "Imported presentation"
                } else {
                    "Imported document"
                };
                entity_labels
                    .entry(document_label.into())
                    .or_insert_with(|| EntityDraft {
                        kind: if document_label == "Imported presentation" {
                            "presentation"
                        } else {
                            "document"
                        }
                        .into(),
                        label: document_label.into(),
                        evidence: first.evidence(),
                    });
            }
        }
        draft.entities.extend(entity_labels.values().cloned());
        if let Some(event) = event {
            let event_label = event.value.clone();
            for candidate in &accepted {
                let idx = candidates
                    .iter()
                    .position(|c| std::ptr::eq(c, *candidate))
                    .unwrap_or(0);
                let value = match candidate.kind {
                    CandidateKind::Date | CandidateKind::UnknownDate => {
                        let choice = validated
                            .event_date
                            .as_ref()
                            .map(|(choice, _)| choice.as_str())
                            .unwrap_or("none");
                        let selected_candidate = candidates
                            .iter()
                            .enumerate()
                            .find(|(i, _)| format!("date_{i}") == choice)
                            .map(|(_, c)| c);
                        if selected_candidate
                            .is_some_and(|selected| std::ptr::eq(selected, *candidate))
                        {
                            Some(("event_date", candidate.value.as_str()))
                        } else {
                            None
                        }
                    }
                    CandidateKind::Count => Some(("visit_count", candidate.value.as_str())),
                    CandidateKind::Duration => Some(("duration", candidate.value.as_str())),
                    CandidateKind::Person
                        if matches!(
                            validated.entity_roles.get(&idx).unwrap().0.as_str(),
                            "observer" | "attendee"
                        ) =>
                    {
                        Some(("observer", candidate.value.as_str()))
                    }
                    CandidateKind::Place
                        if validated.entity_roles.get(&idx).unwrap().0 == "location"
                            || validated
                                .candidate_support
                                .get(&idx)
                                .is_some_and(|support| *support >= SUPPORT_THRESHOLD) =>
                    {
                        Some(("location", candidate.value.as_str()))
                    }
                    _ => None,
                };
                if let Some((property, value)) = value {
                    let mut evidence = candidate.evidence();
                    if candidate.qualifier.is_some() {
                        evidence.qualifier = candidate.qualifier.clone();
                    }
                    draft.facts.push(FactDraft {
                        subject: event_label.clone(),
                        property: property.into(),
                        value: value.into(),
                        evidence: evidence.clone(),
                        record_key: None,
                    });
                    if property == "observer" {
                        let role = validated.entity_roles.get(&idx).unwrap().0.as_str();
                        let to = person_labels
                            .get(&idx)
                            .cloned()
                            .unwrap_or_else(|| format!("{} ({})", value, role));
                        draft.relationships.push(RelationshipDraft {
                            from: event_label.clone(),
                            to,
                            kind: if role == "attendee" {
                                "attended_by"
                            } else {
                                "observed_by"
                            }
                            .into(),
                            qualifier: evidence.qualifier.clone(),
                            evidence,
                        });
                    } else if property == "location" {
                        draft.relationships.push(RelationshipDraft {
                            from: event_label.clone(),
                            to: value.into(),
                            kind: "occurred_at".into(),
                            qualifier: evidence.qualifier.clone(),
                            evidence,
                        });
                    }
                }
            }
        }
        for candidate in accepted
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Reference)
        {
            draft.facts.push(FactDraft {
                subject: candidate.value.clone(),
                property: "acquired_content".into(),
                value: "not acquired".into(),
                evidence: candidate.evidence(),
                record_key: None,
            });
        }
        for candidate in accepted.iter().filter(|candidate| {
            matches!(
                candidate.kind,
                CandidateKind::OfficeStatement | CandidateKind::PhotoStatement
            )
        }) {
            let channel = office_channel_at(source_text, candidate.start).unwrap_or_default();
            let subject = if photo_mode {
                "Imported photo"
            } else if matches!(
                channel.as_str(),
                "slide_text" | "slide_text_table" | "speaker_notes"
            ) {
                "Imported presentation"
            } else {
                "Imported document"
            };
            let property = match channel.as_str() {
                "observed_pixels" => "image_pixels",
                "file_metadata" => "image_metadata",
                "supplied_caption" => "image_caption",
                "ocr" => "image_text",
                "generated_interpretation" => "image_interpretation",
                "table" => "table_row",
                "slide_text" => "visible_slide_text",
                "slide_text_table" => "visible_slide_table_row",
                "speaker_notes" => "speaker_note",
                _ => "document_statement",
            };
            draft.facts.push(FactDraft {
                subject: subject.into(),
                property: property.into(),
                value: candidate.value.clone(),
                evidence: candidate.evidence(),
                record_key: if photo_mode {
                    Some(photo_record_key(source_text, candidate.start, &channel))
                } else {
                    office_record_key(
                        source_text,
                        candidate.start,
                        channel.as_str(),
                        &candidate.value,
                    )
                },
            });
        }
        for candidate in accepted
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Tag)
        {
            let subject = event
                .map(|event| event.value.clone())
                .or_else(|| draft.entities.first().map(|entity| entity.label.clone()));
            if let Some(subject) = subject {
                draft.tags.push(TagDraft {
                    subject,
                    label: candidate.value.clone(),
                    evidence: candidate.evidence(),
                });
            }
        }
        Ok(draft)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CandidateKind {
    Event,
    Person,
    Place,
    Date,
    UnknownDate,
    Count,
    Duration,
    Reference,
    Tag,
    OfficeStatement,
    PhotoStatement,
}

#[derive(Clone)]
struct Candidate {
    kind: CandidateKind,
    value: String,
    quote: String,
    start: usize,
    end: usize,
    qualifier: Option<String>,
    origin: String,
}

#[derive(Clone)]
struct SourceSpan {
    quote: String,
    start: usize,
    end: usize,
}

struct ReportDateCandidate {
    iso: Option<String>,
    evidence: EvidenceDraft,
}

struct ReportRevisionCandidate {
    revision: u64,
    evidence: EvidenceDraft,
}

#[derive(Default)]
struct ValidatedAnswers {
    event_identity: Option<(String, f64)>,
    event_date: Option<(String, f64)>,
    entity_roles: HashMap<usize, (String, f64)>,
    candidate_support: HashMap<usize, f64>,
    person_identity: Vec<(String, f64)>,
    source_update: Option<SourceUpdateDraft>,
}

fn validate_answers(
    source_text: &str,
    candidates: &[Candidate],
    pairs: &[(&Candidate, &Candidate)],
    answers: &serde_json::Map<String, Value>,
    update_spans: &[SourceSpan],
    report_dates: &[ReportDateCandidate],
    report_revisions: &[ReportRevisionCandidate],
) -> std::result::Result<ValidatedAnswers, ProviderError> {
    let mut validated = ValidatedAnswers::default();
    if candidates
        .iter()
        .any(|candidate| candidate.kind == CandidateKind::Event)
    {
        validated.event_identity = Some(validated_choice(
            answers,
            "event_identity",
            &["event", "not_event", "unclear"],
        )?);
    }
    let mut date_choices = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            matches!(
                candidate.kind,
                CandidateKind::Date | CandidateKind::UnknownDate
            )
        })
        .map(|(index, _)| format!("date_{index}"))
        .collect::<Vec<_>>();
    if !date_choices.is_empty() {
        date_choices.push("none".into());
        let allowed = date_choices.iter().map(String::as_str).collect::<Vec<_>>();
        validated.event_date = Some(validated_choice(answers, "event_date", &allowed)?);
    }
    for (index, candidate) in candidates.iter().enumerate() {
        if matches!(candidate.kind, CandidateKind::Person | CandidateKind::Place) {
            validated.entity_roles.insert(
                index,
                validated_choice(
                    answers,
                    &format!("relation_{index}"),
                    &["observer", "attendee", "location", "other", "none"],
                )?,
            );
        }
        if !matches!(candidate.kind, CandidateKind::Event | CandidateKind::Person) {
            validated
                .candidate_support
                .insert(index, validated_noul(answers, &format!("support_{index}"))?);
        }
    }
    for index in 0..pairs.len() {
        validated.person_identity.push(validated_choice(
            answers,
            &format!("person_identity_{index}"),
            &["distinct", "uncertain", "same"],
        )?);
    }
    let (role, certainty) = validated_choice(
        answers,
        "source_update_role",
        &[
            "complete_replacement",
            "supplement",
            "conditional",
            "targeted_correction",
            "unknown",
        ],
    )?;
    let span_options = update_spans
        .iter()
        .enumerate()
        .map(|(index, _)| format!("span_{index}"))
        .collect::<Vec<_>>();
    let selected_span = validated_choice(
        answers,
        "source_update_evidence",
        &span_options.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let selected_span_index = selected_span
        .0
        .strip_prefix("span_")
        .and_then(|index| index.parse::<usize>().ok())
        .and_then(|index| update_spans.get(index))
        .ok_or_else(|| ProviderError::recoverable("Jev source-update evidence selected an unknown source span; semantic work will retry automatically.".into()))?;
    let role = match role.as_str() {
        "complete_replacement" => SourceUpdateRole::CompleteReplacement,
        "supplement" => SourceUpdateRole::Supplement,
        "conditional" => SourceUpdateRole::Conditional,
        "targeted_correction" => SourceUpdateRole::TargetedCorrection,
        "unknown" => SourceUpdateRole::Unknown,
        _ => unreachable!(),
    };
    let source_date_options = report_dates
        .iter()
        .enumerate()
        .map(|(index, _)| format!("source_date_{index}"))
        .chain(std::iter::once("none".to_owned()))
        .collect::<Vec<_>>();
    let (selected_date, date_certainty) = validated_choice(
        answers,
        "source_order_date",
        &source_date_options
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )?;
    let (source_date, source_date_evidence) = if selected_date == "none" {
        (None, None)
    } else {
        let index = selected_date
            .strip_prefix("source_date_")
            .and_then(|index| index.parse::<usize>().ok())
            .ok_or_else(|| ProviderError::recoverable("Jev returned an invalid source-date option; semantic work will retry automatically.".into()))?;
        let candidate = report_dates.get(index).ok_or_else(|| ProviderError::recoverable("Jev selected a source date outside the candidate set; semantic work will retry automatically.".into()))?;
        let value = candidate.iso.clone().ok_or_else(|| ProviderError::recoverable("Jev selected an invalid calendar date as source order; semantic work will retry automatically.".into()))?;
        (Some(value), Some(candidate.evidence.clone()))
    };
    let revision_options = report_revisions
        .iter()
        .enumerate()
        .map(|(index, _)| format!("source_revision_{index}"))
        .chain(std::iter::once("none".to_owned()))
        .collect::<Vec<_>>();
    let (selected_revision, revision_certainty) = validated_choice(
        answers,
        "source_order_revision",
        &revision_options
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )?;
    let (source_revision, source_revision_evidence) = if selected_revision == "none" {
        (None, None)
    } else {
        let index = selected_revision
            .strip_prefix("source_revision_")
            .and_then(|index| index.parse::<usize>().ok())
            .ok_or_else(|| ProviderError::recoverable("Jev returned an invalid source-revision option; semantic work will retry automatically.".into()))?;
        let candidate = report_revisions.get(index).ok_or_else(|| ProviderError::recoverable("Jev selected a source revision outside the candidate set; semantic work will retry automatically.".into()))?;
        (Some(candidate.revision), Some(candidate.evidence.clone()))
    };
    validated.source_update = Some(SourceUpdateDraft {
        role,
        evidence: EvidenceDraft {
            quote: selected_span_index.quote.clone(),
            byte_start: selected_span_index.start,
            byte_end: selected_span_index.end,
            origin: origin_for(
                source_text,
                selected_span_index.start,
                selected_span_index.end,
            ),
            qualifier: None,
            offset_basis: None,
            source_location: None,
        },
        certainty,
        source_date,
        source_date_evidence,
        source_date_certainty: date_certainty,
        source_revision,
        source_revision_evidence,
        source_revision_certainty: revision_certainty,
    });
    Ok(validated)
}

fn validated_choice(
    answers: &serde_json::Map<String, Value>,
    key: &str,
    allowed: &[&str],
) -> std::result::Result<(String, f64), ProviderError> {
    let answer = answers
        .get(key)
        .filter(|answer| answer.get("type").and_then(Value::as_str) == Some("choice"))
        .ok_or_else(|| {
            ProviderError::recoverable(format!(
                "Jev omitted or returned the wrong answer type for `{key}`; semantic work will retry automatically."
            ))
        })?;
    let choice = answer
        .get("choice")
        .and_then(Value::as_str)
        .filter(|choice| allowed.contains(choice))
        .ok_or_else(|| {
            ProviderError::recoverable(format!(
                "Jev returned an invalid choice for `{key}` (allowed: {}); semantic work will retry automatically.",
                allowed.join(", ")
            ))
        })?;
    let probability = choice_probability_from_answer(answer, choice).ok_or_else(|| {
        ProviderError::recoverable(format!(
            "Jev omitted or returned an invalid probability for `{key}` choice `{choice}`; semantic work will retry automatically."
        ))
    })?;
    Ok((choice.to_owned(), probability))
}

fn validated_noul(
    answers: &serde_json::Map<String, Value>,
    key: &str,
) -> std::result::Result<f64, ProviderError> {
    answers
        .get(key)
        .filter(|answer| answer.get("type").and_then(Value::as_str) == Some("noul"))
        .and_then(|answer| answer.get("noul"))
        .and_then(Value::as_f64)
        .filter(|probability| probability.is_finite() && (0.0..=1.0).contains(probability))
        .ok_or_else(|| {
            ProviderError::recoverable(format!(
                "Jev omitted or returned an invalid support probability for `{key}`; semantic work will retry automatically."
            ))
        })
}

fn source_spans(text: &str) -> Vec<SourceSpan> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (index, character) in text.char_indices() {
        let end = index + character.len_utf8();
        let sentence_end = matches!(character, '.' | '!' | '?')
            && text[end..].chars().next().is_none_or(char::is_whitespace);
        if character == '\n' || sentence_end {
            push_source_span(text, start, end, &mut spans);
            start = end;
        }
    }
    push_source_span(text, start, text.len(), &mut spans);
    spans
}

fn push_source_span(text: &str, start: usize, end: usize, spans: &mut Vec<SourceSpan>) {
    let Some(raw) = text.get(start..end) else {
        return;
    };
    let leading = raw.len() - raw.trim_start().len();
    let trailing_end = raw.trim_end().len();
    if trailing_end <= leading {
        return;
    }
    let byte_start = start + leading;
    let byte_end = start + trailing_end;
    if text.is_char_boundary(byte_start) && text.is_char_boundary(byte_end) {
        spans.push(SourceSpan {
            quote: text[byte_start..byte_end].to_owned(),
            start: byte_start,
            end: byte_end,
        });
    }
}

fn report_date_candidates(text: &str) -> Vec<ReportDateCandidate> {
    if text.starts_with("[PHOTO ") {
        return Vec::new();
    }
    let date_re = regex(
        r"(?i)\b(?:[0-9]{4}-[0-9]{2}-[0-9]{2}|(?:January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)\s+[0-9]{1,2},?\s+[0-9]{4})\b",
    );
    date_re
        .find_iter(text)
        .map(|matched| ReportDateCandidate {
            iso: normalize_source_date(matched.as_str()),
            evidence: EvidenceDraft {
                quote: matched.as_str().to_owned(),
                byte_start: matched.start(),
                byte_end: matched.end(),
                origin: origin_for(text, matched.start(), matched.end()),
                qualifier: None,
                offset_basis: None,
                source_location: None,
            },
        })
        .collect()
}

fn report_revision_candidates(text: &str) -> Vec<ReportRevisionCandidate> {
    if text.starts_with("[PHOTO ") {
        return Vec::new();
    }
    let revision_re = regex(r"(?i)\brevision\s+([0-9]+)\b");
    revision_re
        .captures_iter(text)
        .filter_map(|captures| {
            let full = captures.get(0)?;
            let revision = captures.get(1)?.as_str().parse::<u64>().ok()?;
            (revision > 0).then(|| ReportRevisionCandidate {
                revision,
                evidence: EvidenceDraft {
                    quote: full.as_str().to_owned(),
                    byte_start: full.start(),
                    byte_end: full.end(),
                    origin: origin_for(text, full.start(), full.end()),
                    qualifier: None,
                    offset_basis: None,
                    source_location: None,
                },
            })
        })
        .collect()
}

fn normalize_source_date(value: &str) -> Option<String> {
    if let Some(captures) = regex(r"^([0-9]{4})-([0-9]{2})-([0-9]{2})$").captures(value) {
        let year = captures.get(1)?.as_str().parse::<u32>().ok()?;
        let month = captures.get(2)?.as_str().parse::<u32>().ok()?;
        let day = captures.get(3)?.as_str().parse::<u32>().ok()?;
        if valid_calendar_date(year, month, day) {
            return Some(value.to_owned());
        }
        return None;
    }
    let captures = regex(r"(?i)^\s*(January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)\s+([0-9]{1,2}),?\s+([0-9]{4})\s*$")
        .captures(value)?;
    let month = match captures.get(1)?.as_str().to_lowercase().as_str() {
        "january" | "jan" => 1,
        "february" | "feb" => 2,
        "march" | "mar" => 3,
        "april" | "apr" => 4,
        "may" => 5,
        "june" | "jun" => 6,
        "july" | "jul" => 7,
        "august" | "aug" => 8,
        "september" | "sep" | "sept" => 9,
        "october" | "oct" => 10,
        "november" | "nov" => 11,
        "december" | "dec" => 12,
        _ => return None,
    };
    let day = captures.get(2)?.as_str().parse::<u32>().ok()?;
    let year = captures.get(3)?.as_str().parse::<u32>().ok()?;
    valid_calendar_date(year, month, day).then(|| format!("{year:04}-{month:02}-{day:02}"))
}

fn valid_calendar_date(year: u32, month: u32, day: u32) -> bool {
    if year == 0 || !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    day <= max_day
}

impl Candidate {
    fn evidence(&self) -> EvidenceDraft {
        EvidenceDraft {
            quote: self.quote.clone(),
            byte_start: self.start,
            byte_end: self.end,
            origin: self.origin.clone(),
            qualifier: self.qualifier.clone(),
            offset_basis: None,
            source_location: None,
        }
    }
}

fn office_statement_candidates(text: &str) -> Vec<Candidate> {
    let mut result = Vec::new();
    let mut line_start = 0usize;
    for raw_line in text.split_inclusive('\n') {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        if let Some((channel, body_offset)) = office_line_channel(line) {
            if matches!(
                channel,
                "main_document"
                    | "table"
                    | "slide_text"
                    | "slide_text_table"
                    | "speaker_notes"
                    | "observed_pixels"
                    | "file_metadata"
                    | "supplied_caption"
                    | "ocr"
                    | "generated_interpretation"
            ) {
                let body = &line[body_offset..];
                let leading = body.len() - body.trim_start().len();
                let quote = body.trim();
                if !quote.is_empty() {
                    let start = line_start + body_offset + leading;
                    let end = start + quote.len();
                    let origin = match channel {
                        "table" => "table_cell",
                        "slide_text" => "slide_visible",
                        "slide_text_table" => "slide_table_cell",
                        "speaker_notes" => "speaker_note",
                        "observed_pixels"
                        | "file_metadata"
                        | "supplied_caption"
                        | "ocr"
                        | "generated_interpretation" => channel,
                        _ => "document_body",
                    };
                    result.push(Candidate {
                        kind: if line.starts_with("[PHOTO ") { CandidateKind::PhotoStatement } else { CandidateKind::OfficeStatement },
                        value: quote.to_owned(),
                        quote: quote.to_owned(),
                        start,
                        end,
                        qualifier: if line.starts_with("[PHOTO ") { Some(match channel {
                            "file_metadata" => "unauthenticated file metadata; capture date unknown",
                            "supplied_caption" => "unverified supplied caption; not observed pixels or authenticated capture date",
                            "ocr" => "OCR transcription; confidence and region retained; not a verified event or identity",
                            "generated_interpretation" => "uncertain classifier interpretation; not a verified pixel observation",
                            _ => "decoded dimensions; named identities unknown",
                        }.into()) } else { office_qualification(quote) },
                        origin: origin.into(),
                    });
                }
            }
        }
        line_start += raw_line.len();
    }
    result
}

fn office_line_channel(line: &str) -> Option<(&str, usize)> {
    let closing = line.find("] ")?;
    let marker = line.get(1..closing)?;
    let channel = marker
        .strip_prefix("PPTX ")
        .or_else(|| marker.strip_prefix("DOCX "))
        .or_else(|| marker.strip_prefix("PHOTO "))?;
    let (_, channel) = channel.rsplit_once("; channel=")?;
    Some((channel, closing + 2))
}

fn office_channel_at(text: &str, offset: usize) -> Option<String> {
    let start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let end = text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index);
    office_line_channel(&text[start..end]).map(|(channel, _)| channel.to_owned())
}

fn photo_record_key(text: &str, offset: usize, channel: &str) -> String {
    let start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line = text[start..].split('\n').next().unwrap_or_default();
    let marker = line.split(';').next().unwrap_or("PHOTO whole image");
    let qualifier_key = if channel == "generated_interpretation" {
        line.split("Classifier prediction: ")
            .nth(1)
            .unwrap_or_default()
            .split(" (")
            .next()
            .unwrap_or_default()
    } else {
        marker
    };
    format!("photo:{channel}:{}", office_key_slug(qualifier_key))
}

fn office_record_key(text: &str, offset: usize, channel: &str, quote: &str) -> Option<String> {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[offset..]
        .find('\n')
        .map_or(text.len(), |index| offset + index);
    let marker = text[line_start..line_end]
        .find(']')
        .and_then(|closing| text[line_start + 1..line_start + closing].split_once(' '))
        .map(|(_, marker)| marker)?;
    let heading = office_heading_context(text, line_start);

    // OOXML paragraph and shape IDs are stable identities owned by the source package.
    // Keep the part path because shape IDs are scoped to their XML part.
    let object_identity = office_ooxml_object_identity(marker);
    if let Some(object_identity) = object_identity.as_deref() {
        if channel == "slide_text_table" {
            // A graphic-frame ID identifies the table, not an individual row. Combine it
            // with the stable field and row subject so separate rows do not collide.
        } else {
            return Some(format!(
                "office-object:{}:{}:{}",
                office_key_slug(&heading.unwrap_or_default()),
                channel,
                office_key_slug(object_identity)
            ));
        }
    }

    if matches!(channel, "table" | "slide_text_table") {
        let (field, subject) = quote.split(';').next()?.split_once(':')?;
        let field = office_key_slug(field);
        let subject = office_key_slug(subject);
        if !field.is_empty() && !subject.is_empty() {
            return Some(format!(
                "office-field:{}:{}:{}:{}-{}",
                channel,
                office_key_slug(&heading.unwrap_or_default()),
                object_identity
                    .as_deref()
                    .map(office_key_slug)
                    .unwrap_or_default(),
                field,
                subject,
            ));
        }
    }

    if matches!(channel, "slide_text" | "slide_text_table") {
        if let Some((field, _)) = quote.split_once(':') {
            let field = office_key_slug(field);
            if !field.is_empty() {
                return Some(format!(
                    "office-slide-field:{}:{}",
                    office_key_slug(&heading.unwrap_or_default()),
                    field
                ));
            }
        }
    }

    // For projected prose without a stable package ID, use a short source-grounded subject
    // phrase under its nearest heading. Stop at common predicates so changed readings are
    // excluded from the identity.
    let subject = office_leading_subject(quote)?;
    Some(format!(
        "office-subject:{}:{}:{}",
        channel,
        office_key_slug(&heading.unwrap_or_default()),
        subject
    ))
}

fn office_ooxml_object_identity(marker: &str) -> Option<String> {
    let (_, object) = marker.split_once("OOXML part ")?;
    let (part, details) = object.split_once(' ')?;
    for identifier in ["paragraph id ", "shape id "] {
        if let Some((_, id)) = details.split_once(identifier) {
            let id = id.split_whitespace().next()?;
            return Some(format!("{part}:{identifier}{id}"));
        }
    }
    None
}

fn office_heading_context(text: &str, line_start: usize) -> Option<String> {
    let mut cursor = 0usize;
    let mut nearest = None;
    for raw_line in text.split_inclusive('\n') {
        if cursor >= line_start {
            break;
        }
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        if let Some(("main_document", body_offset)) = office_line_channel(line) {
            if let Some(marker_end) = line.find("] ") {
                let marker = &line[1..marker_end];
                if marker.contains("style Title") || marker.contains("style Heading") {
                    nearest = Some(line[body_offset..].trim().to_owned());
                }
            }
        }
        cursor += raw_line.len();
    }
    nearest
}

fn office_leading_subject(text: &str) -> Option<String> {
    let mut words = Vec::new();
    let stop = [
        "a",
        "an",
        "the",
        "this",
        "that",
        "these",
        "those",
        "unconfirmed",
        "unverified",
        "provisional",
        "conditional",
        "reported",
        "possibly",
        "estimated",
        "is",
        "are",
        "was",
        "were",
        "be",
        "been",
        "being",
        "left",
        "remains",
        "remained",
        "measured",
        "measures",
        "recorded",
        "reports",
        "shows",
        "showed",
        "suggests",
        "suggested",
        "confirms",
        "confirmed",
        "indicates",
        "indicated",
        "occurred",
        "happened",
        "has",
        "have",
        "had",
    ];
    for token in text
        .split(|character: char| !character.is_alphanumeric() && character != '-')
        .filter(|token| !token.is_empty())
    {
        let word = token.to_lowercase();
        if word.chars().all(char::is_numeric) {
            continue;
        }
        if stop.contains(&word.as_str()) {
            if words.is_empty() {
                continue;
            }
            break;
        }
        words.push(word);
        if words.len() == 3 {
            break;
        }
    }
    (!words.is_empty()).then(|| words.join("-"))
}

fn office_key_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_lowercase().collect::<String>()
            } else {
                "-".to_owned()
            }
        })
        .collect::<String>();
    slug.split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn office_qualification(text: &str) -> Option<String> {
    let matches = regex(r"(?i)\b(?:gauge\s+calibration\s+pending|calibration\s+pending|not\s+a\s+record(?:[\s-]+[\w-]+){0,5}|not\s+confirmed|not\s+verified|unconfirmed(?:[\s-]+[\w-]+){0,2}|unverified(?:[\s-]+[\w-]+){0,2}|possibly|possible|estimated(?:[\s-]+[\w-]+){0,2}|estimate|excluding(?:[\s-]+[\w-]+){0,3}|except(?:[\s-]+[\w-]+){0,3}|alleged|reported|provisional(?:[\s-]+[\w-]+){0,6}|uncalibrated(?:[\s-]+[\w-]+){0,2}|conditional|pending(?:[\s-]+[\w-]+){0,6}|unknown|may\s+be\s+\w+)")
        .find_iter(text)
        .map(|matched| matched.as_str().trim().trim_end_matches(','))
        .collect::<Vec<_>>();
    (!matches.is_empty()).then(|| matches.join("; "))
}

fn candidates(text: &str) -> Vec<Candidate> {
    let mut result = Vec::new();
    let event_re = regex(r"\b(?:V[0-9]+|(?i:observation|survey|event)\s+(?-i:[A-Z][A-Z0-9_-]*))\b");
    for m in event_re.find_iter(text) {
        let id = m.as_str().to_owned();
        let (start, end) = sentence_bounds(text, m.start(), m.end());
        let quote = text[start..end].to_owned();
        let origin = origin_for(text, m.start(), m.end());
        let value = if id.starts_with('V') {
            format!("Observation {id}")
        } else {
            let mut words = id.split_whitespace();
            let kind = words.next().unwrap_or("Observation");
            let id = words.next().unwrap_or("");
            format!("{} {}", capitalize(kind), id)
        };
        result.push(Candidate {
            kind: CandidateKind::Event,
            value,
            quote,
            start,
            end,
            qualifier: None,
            origin,
        });
    }
    let tag_re = regex(r"#[\p{L}\p{N}_-]+\b");
    for m in tag_re.find_iter(text) {
        let value = m.as_str().trim_start_matches('#').to_owned();
        result.push(Candidate {
            kind: CandidateKind::Tag,
            value,
            quote: m.as_str().to_owned(),
            start: m.start(),
            end: m.end(),
            qualifier: None,
            origin: origin_for(text, m.start(), m.end()),
        });
    }
    let date_re = regex(
        r"\b(?:Jan(?:uary)?|Feb(?:ruary)?|Mar(?:ch)?|Apr(?:il)?|May|Jun(?:e)?|Jul(?:y)?|Aug(?:ust)?|Sep(?:tember)?|Oct(?:ober)?|Nov(?:ember)?|Dec(?:ember)?)\s+[0-9]{1,2},?\s+[0-9]{4}\b",
    );
    for m in date_re.find_iter(text) {
        let value = m.as_str().to_owned();
        let origin = origin_for(text, m.start(), m.end());
        let (start, end) = sentence_bounds(text, m.start(), m.end());
        let quote = text[start..end].to_owned();
        result.push(Candidate {
            kind: CandidateKind::Date,
            value,
            quote,
            start,
            end,
            qualifier: None,
            origin,
        });
    }
    let unknown_date_re = regex(
        r"(?i)\b(?:(?:the\s+)?(?:observation\s+)?date\s+(?:is\s+unknown|was\s+unknown|was\s+not\s+supplied)|no\s+(?:observation\s+)?date\s+was\s+supplied)\b",
    );
    for m in unknown_date_re.find_iter(text) {
        let origin = origin_for(text, m.start(), m.end());
        let (start, end) = sentence_bounds(text, m.start(), m.end());
        let quote = text[start..end].to_owned();
        result.push(Candidate {
            kind: CandidateKind::UnknownDate,
            value: "unknown".into(),
            quote,
            start,
            end,
            qualifier: Some("date not supplied or stated unknown".into()),
            origin,
        });
    }
    let count_re = regex(
        r"(?i)\b(?:(?:[A-Z][a-z]+\s+)?estimated\s+)?(?:possibly\s+(?:counted\s+)?)?(?:[0-9]+\s+visits?|visits?\s*:\s*(?:possibly\s+)?[0-9]+)\b",
    );
    for m in count_re.find_iter(text) {
        let origin = origin_for(text, m.start(), m.end());
        let match_quote = m.as_str().to_owned();
        let (start, end) = sentence_bounds(text, m.start(), m.end());
        let quote = text[start..end].to_owned();
        let lowered = quote.to_lowercase();
        let trailing = text[m.end()..]
            .chars()
            .take(90)
            .collect::<String>()
            .to_lowercase();
        let qualifier = if lowered.contains("possibly")
            && (lowered.contains("estimated")
                || trailing.contains("estimate")
                || trailing.contains("not a confirmed"))
        {
            Some("possibly; estimate, not confirmed".into())
        } else if lowered.contains("possibly") {
            Some("possibly".into())
        } else if lowered.contains("estimate")
            || trailing.contains("estimate")
            || trailing.contains("not a confirmed")
        {
            Some("estimate; not confirmed".into())
        } else {
            None
        };
        let value = if match_quote.to_lowercase().contains("visits:") {
            regex(r"[0-9]+")
                .find(&match_quote)
                .map(|number| format!("{} visits", number.as_str()))
                .unwrap_or_default()
        } else {
            regex(r"(?i)^(?:(?:[A-Z][a-z]+\s+)?estimated\s+)?(?:possibly\s+(?:counted\s+)?)?")
                .replace(&match_quote, "")
                .to_string()
        };
        result.push(Candidate {
            kind: CandidateKind::Count,
            value,
            quote,
            start,
            end,
            qualifier,
            origin,
        });
    }
    // A narrow, explicitly corrective phrase is one candidate: the old number
    // after "not" is contrastive evidence, not a second current count.
    // Keep the entire sentence as provenance so Jev can confirm both the
    // target field and the rejected prior value before publication.
    let count_correction_re = regex(
        r"(?i)\bvisit\s+total\s+(?:should\s+)?(?:read|be\s+corrected\s+to)\s+([0-9]+)\s*,?\s+not\s+([0-9]+)\b",
    );
    for matched in count_correction_re.captures_iter(text) {
        let Some(whole) = matched.get(0) else {
            continue;
        };
        let Some(corrected) = matched.get(1) else {
            continue;
        };
        let (start, end) = sentence_bounds(text, whole.start(), whole.end());
        let quote = text[start..end].to_owned();
        let origin = origin_for(text, whole.start(), whole.end());
        result.retain(|candidate| {
            candidate.kind != CandidateKind::Count
                || candidate.end <= start
                || candidate.start >= end
        });
        result.push(Candidate {
            kind: CandidateKind::Count,
            value: format!("{} visits", corrected.as_str()),
            quote,
            start,
            end,
            qualifier: None,
            origin,
        });
    }
    let duration_re = regex(r"(?i)\b[0-9]+\s*(?:minutes?|hours?|seconds?|days?)\b");
    for m in duration_re.find_iter(text) {
        let matched = m.as_str();
        let value = regex(r"(?i)^([0-9]+)\s*(minutes?|hours?|seconds?|days?)$")
            .captures(matched)
            .map(|captures| {
                format!(
                    "{} {}",
                    captures.get(1).unwrap().as_str(),
                    captures.get(2).unwrap().as_str().to_lowercase()
                )
            })
            .unwrap_or_else(|| matched.to_owned());
        let origin = origin_for(text, m.start(), m.end());
        let (start, end) = sentence_bounds(text, m.start(), m.end());
        let quote = text[start..end].to_owned();
        result.push(Candidate {
            kind: CandidateKind::Duration,
            value,
            quote,
            start,
            end,
            qualifier: None,
            origin,
        });
    }
    let person_re = regex(
        r"\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,2})(?:,\s+[^.;]{0,48})?\s+(?i:observed|attended|filed|estimated|recorded|reported|counted)\b|\b(?i:observer)\s*:?\s*([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,2})|\b(?:a different person named|another person named)\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,2})",
    );
    for captures in person_re.captures_iter(text) {
        let m = captures.get(0).unwrap();
        let name = captures
            .get(1)
            .or_else(|| captures.get(2))
            .or_else(|| captures.get(3))
            .unwrap();
        if matches!(
            name.as_str().to_lowercase().as_str(),
            "i" | "we" | "you" | "he" | "she" | "they" | "it"
        ) {
            continue;
        }
        let quote = m.as_str().to_owned();
        result.push(Candidate {
            kind: CandidateKind::Person,
            value: name.as_str().to_owned(),
            quote,
            start: m.start(),
            end: m.end(),
            qualifier: None,
            origin: origin_for(text, m.start(), m.end()),
        });
    }
    let place_re =
        regex(r"\b(?i:at|location:?|occurred at)\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+){0,2})");
    for captures in place_re.captures_iter(text) {
        let m = captures.get(0).unwrap();
        let value = captures.get(1).unwrap();
        let origin = origin_for(text, m.start(), m.end());
        let (start, end) = sentence_bounds(text, m.start(), m.end());
        let quote = text[start..end].to_owned();
        let context = &text[start..end];
        let qualifier = context
            .to_lowercase()
            .contains("possibly")
            .then(|| "possibly".to_owned());
        result.push(Candidate {
            kind: CandidateKind::Place,
            value: value.as_str().to_owned(),
            quote,
            start,
            end,
            qualifier,
            origin,
        });
    }
    let url_re = regex(r"https?://[^\s)>]+");
    for m in url_re.find_iter(text) {
        let office_line = text[..m.start()].rfind('\n').map_or(0, |index| index + 1);
        let office_end = text[m.start()..]
            .find('\n')
            .map_or(text.len(), |index| m.start() + index);
        let (start, end) = if let Some(("reference", content_start)) =
            office_line_channel(&text[office_line..office_end])
        {
            (office_line + content_start, office_end)
        } else {
            sentence_bounds(text, m.start(), m.end())
        };
        result.push(Candidate {
            kind: CandidateKind::Reference,
            value: m.as_str().trim_end_matches(['.', ',']).to_owned(),
            quote: text[start..end].to_owned(),
            start,
            end,
            qualifier: None,
            origin: origin_for(text, m.start(), m.end()),
        });
    }
    // Keep candidates in source order; the provider reports incomplete coverage over 64.
    result.sort_by_key(|candidate| (candidate.start, candidate.end));
    let mut normalized = Vec::<Candidate>::new();
    for candidate in result {
        if candidate.kind == CandidateKind::Event
            && normalized.iter().any(|existing| {
                existing.kind == CandidateKind::Event && existing.value == candidate.value
            })
        {
            continue;
        }
        if candidate.kind == CandidateKind::Person {
            if let Some(existing) = normalized.iter_mut().find(|existing| {
                existing.kind == CandidateKind::Person
                    && existing.value.eq_ignore_ascii_case(&candidate.value)
                    && existing.start < candidate.end
                    && candidate.start < existing.end
            }) {
                existing.start = existing.start.min(candidate.start);
                existing.end = existing.end.max(candidate.end);
                existing.quote = text[existing.start..existing.end].to_owned();
                existing.origin = origin_for(text, existing.start, existing.end);
                continue;
            }
        }
        normalized.push(candidate);
    }
    normalized
}

fn correction_candidates(text: &str, candidates: &[Candidate]) -> Vec<CorrectionCandidateDraft> {
    let correction_re = regex(
        r"(?i)\bvisit\s+total\s+(?:should\s+)?(?:read|be\s+corrected\s+to)\s+([0-9]+)\s*,?\s+not\s+([0-9]+)\b",
    );
    correction_re
        .captures_iter(text)
        .filter_map(|captures| {
            let whole = captures.get(0)?;
            let corrected = captures.get(1)?;
            let previous = captures.get(2)?;
            let (start, end) = sentence_bounds(text, whole.start(), whole.end());
            let quote = &text[start..end];
            let event_label = candidates
                .iter()
                .find(|candidate| {
                    candidate.kind == CandidateKind::Event && candidate.quote == quote
                })?
                .value
                .clone();
            Some(CorrectionCandidateDraft {
                event_label,
                property: "visit_count".into(),
                previous_value: format!("{} visits", previous.as_str()),
                corrected_value: format!("{} visits", corrected.as_str()),
                evidence: EvidenceDraft {
                    quote: quote.to_owned(),
                    byte_start: start,
                    byte_end: end,
                    origin: origin_for(text, whole.start(), whole.end()),
                    qualifier: None,
                    offset_basis: None,
                    source_location: None,
                },
            })
        })
        .collect()
}

pub(crate) fn correction_candidates_from_text(text: &str) -> Vec<CorrectionCandidateDraft> {
    let candidates = candidates(text);
    correction_candidates(text, &candidates)
}

fn person_pairs(candidates: &[Candidate]) -> Vec<(&Candidate, &Candidate)> {
    let people = candidates
        .iter()
        .filter(|candidate| candidate.kind == CandidateKind::Person)
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for i in 0..people.len() {
        for j in (i + 1)..people.len() {
            if people[i].value.eq_ignore_ascii_case(&people[j].value) {
                pairs.push((people[i], people[j]));
            }
        }
    }
    pairs
}

fn choice_probability_from_answer(answer: &Value, choice: &str) -> Option<f64> {
    answer
        .get("probabilities")?
        .get(choice)?
        .as_f64()
        .filter(|probability| probability.is_finite() && (0.0..=1.0).contains(probability))
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

fn sentence_bounds(text: &str, start: usize, end: usize) -> (usize, usize) {
    let mut left = text[..start]
        .rfind(['.', '?', '!', '\n'])
        .map_or(0, |index| index + 1);
    let mut right = text[end..]
        .find(['.', '?', '!', '\n'])
        .map_or(text.len(), |index| end + index + 1);
    while left < right {
        let ch = text[left..].chars().next().unwrap();
        if !ch.is_whitespace() {
            break;
        }
        left += ch.len_utf8();
    }
    while right < text.len() && matches!(text[right..].chars().next(), Some('\'' | '"' | '’' | '”'))
    {
        right += text[right..].chars().next().unwrap().len_utf8();
    }
    (left, right)
}

fn origin_for(text: &str, start: usize, end: usize) -> String {
    let left = &text[..start];
    let right = &text[end..];
    let quoted = left.rfind(['“', '"', '‘', '\'']).is_some_and(|open| {
        let mark = left[open..].chars().next().unwrap_or('"');
        right.contains(mark)
    });
    if quoted
        || left
            .rsplit_once(':')
            .is_some_and(|(_, prefix)| prefix.to_lowercase().contains("quoted"))
    {
        "quoted".into()
    } else {
        "observed".into()
    }
}

fn regex(pattern: &str) -> &'static Regex {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<String, &'static Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    if let Some(found) = cache
        .lock()
        .expect("regex cache lock")
        .get(pattern)
        .copied()
    {
        return found;
    }
    let compiled = Box::leak(Box::new(
        Regex::new(pattern).expect("valid built-in candidate pattern"),
    ));
    cache
        .lock()
        .expect("regex cache lock")
        .insert(pattern.to_owned(), compiled);
    compiled
}

fn keychain_credential(account: &str) -> std::result::Result<String, ProviderError> {
    let mut child = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            account,
            "-w",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| ProviderError::recoverable(
            "Jev is waiting for macOS Keychain authorization; semantic work will retry automatically.".into(),
        ))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProviderError::recoverable(
                    "Jev is waiting for macOS Keychain authorization; semantic work will retry automatically.".into(),
                ));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProviderError::recoverable(
                    "Jev could not read the Knowledge Garden key from macOS Keychain; semantic work will retry automatically.".into(),
                ));
            }
        }
    }
    let output = child.wait_with_output().map_err(|_| ProviderError::recoverable(
        "Jev could not read the Knowledge Garden key from macOS Keychain; semantic work will retry automatically.".into(),
    ))?;
    if !output.status.success() {
        return Err(ProviderError::recoverable(
            "Jev is waiting for macOS Keychain authorization; semantic work will retry automatically.".into(),
        ));
    }
    let value = String::from_utf8(output.stdout).map_err(|_| ProviderError::recoverable(
        "The Knowledge Garden Keychain value is not valid text; semantic work will retry automatically.".into(),
    ))?.trim().to_owned();
    if value.is_empty() {
        return Err(ProviderError::recoverable(
            "Jev is waiting for an OpenRouter key in the Knowledge Garden Keychain; semantic work will retry automatically.".into(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod ticket5_tests {
    use super::{candidates, CandidateKind};

    #[test]
    fn explicit_visit_total_correction_selects_the_new_count_only() {
        let text = "For Observation V17: The visit total should read 15, not 12. This note corrects the count only.";
        let counts = candidates(text)
            .into_iter()
            .filter(|candidate| candidate.kind == CandidateKind::Count)
            .collect::<Vec<_>>();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].value, "15 visits");
        assert!(counts[0].quote.contains("not 12"));
    }
}
