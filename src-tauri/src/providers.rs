//! Rust-only semantic provider transports. Request bodies and credentials never enter logs.
use crate::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, ProviderError, RelationshipDraft,
    SemanticDecision, SemanticProvider, TagDraft,
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

    fn evaluate(&self, text: &str, questions: Value) -> std::result::Result<Value, ProviderError> {
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
        let request =
            json!({"model": JEV_MODEL, "state": {"source_text": text}, "questions": questions});
        self.transport.complete(&key, &request)
    }
}

impl SemanticProvider for JevSemanticProvider {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        let candidates = candidates(source_text);
        if candidates.is_empty() {
            return Err(ProviderError::recoverable(
                "Jev has no grounded candidate spans for this text yet; semantic coverage remains incomplete.".into(),
            ));
        }
        if candidates.len() > 64 {
            return Err(ProviderError::recoverable("This source produced more than 64 bounded semantic candidates; semantic coverage remains incomplete rather than silently truncating the source.".into()));
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
                _ => "Does this exact source span support the candidate as a statement made by the source? Do not judge whether the statement is true in the world.",
            };
            questions.insert(format!("support_{i}"), json!({
                "type":"noul",
                "instructions":format!("{judgment} Candidate value: {}; preserved qualification: {}; origin: {}. Exact source span: {}", candidate.value, candidate.qualifier.as_deref().unwrap_or("none stated"), candidate.origin, candidate.quote),
                "criteria":{"true":"The value and its stated meaning are supported by the span", "false":"The value is unsupported, overstates the span, or loses an explicit qualification"}
            }));
        }
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
        let result = self.evaluate(source_text, Value::Object(questions.into_iter().collect()))?;
        let answers = result
            .get("answers")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                ProviderError::recoverable("Jev response has no typed answers.".into())
            })?;
        let validated = validate_answers(&candidates, &pairs, answers)?;
        let mut decisions = Vec::new();
        let model = result
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(JEV_MODEL)
            .to_owned();
        let mut accepted = Vec::new();
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.kind == CandidateKind::Event)
        {
            let (choice, confidence) = validated.event_identity.as_ref().unwrap();
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
            if answer >= SUPPORT_THRESHOLD {
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
        {
            let (choice, probability) = validated.event_identity.as_ref().unwrap();
            return Err(ProviderError::recoverable(format!(
                "No supported event identity was found (Jev choice {choice}, probability {probability:.2}); semantic coverage remains incomplete and will retry automatically."
            )));
        }
        let mut draft = KnowledgeDraft {
            decisions,
            ..KnowledgeDraft::default()
        };
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

#[derive(Default)]
struct ValidatedAnswers {
    event_identity: Option<(String, f64)>,
    event_date: Option<(String, f64)>,
    entity_roles: HashMap<usize, (String, f64)>,
    candidate_support: HashMap<usize, f64>,
    person_identity: Vec<(String, f64)>,
}

fn validate_answers(
    candidates: &[Candidate],
    pairs: &[(&Candidate, &Candidate)],
    answers: &serde_json::Map<String, Value>,
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

impl Candidate {
    fn evidence(&self) -> EvidenceDraft {
        EvidenceDraft {
            quote: self.quote.clone(),
            byte_start: self.start,
            byte_end: self.end,
            origin: self.origin.clone(),
            qualifier: self.qualifier.clone(),
        }
    }
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
        result.push(Candidate {
            kind: CandidateKind::Reference,
            value: m.as_str().trim_end_matches(['.', ',']).to_owned(),
            quote: m.as_str().to_owned(),
            start: m.start(),
            end: m.end(),
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
