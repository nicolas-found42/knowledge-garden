//! Ordinary prose is judged as verbatim source passages, without requiring a
//! fixture-style event identifier or asking Jev to generate labels or facts.
use super::*;

pub(super) fn candidates(text: &str) -> Vec<Candidate> {
    let mut candidates = source_spans(text)
        .into_iter()
        .map(|span| Candidate {
            kind: CandidateKind::ProseStatement,
            value: span.quote.clone(),
            quote: span.quote,
            start: span.start,
            end: span.end,
            qualifier: None,
            origin: origin_for(text, span.start, span.end),
        })
        .collect::<Vec<_>>();
    // ISO calendar dates are copied literally, just like month-name candidates.
    // Calendar validity is deterministic; assignment to an event remains judged.
    for matched in regex(r"\b[0-9]{4}-[0-9]{2}-[0-9]{2}\b").find_iter(text) {
        if chrono::NaiveDate::parse_from_str(matched.as_str(), "%Y-%m-%d").is_err() {
            continue;
        }
        let (start, end) = sentence_bounds(text, matched.start(), matched.end());
        let candidate = Candidate {
            kind: CandidateKind::Date,
            value: matched.as_str().to_owned(),
            quote: text[start..end].to_owned(),
            start,
            end,
            qualifier: None,
            origin: origin_for(text, matched.start(), matched.end()),
        };
        if !candidates.iter().any(|prior| {
            prior.kind == candidate.kind
                && prior.value == candidate.value
                && prior.start == start
                && prior.end == end
        }) {
            candidates.push(candidate);
        }
    }
    candidates
}

fn dates_in<'a>(
    passage: &Candidate,
    candidates: &'a [Candidate],
) -> impl Iterator<Item = (usize, &'a Candidate)> {
    let start = passage.start;
    let end = passage.end;
    candidates.iter().enumerate().filter(move |(_, candidate)| {
        // Source passages trim line endings; date context may include them.
        // Compare the meaningful context while retaining its exact evidence.
        let date_end = candidate.start + candidate.quote.trim_end().len();
        matches!(
            candidate.kind,
            CandidateKind::Date | CandidateKind::UnknownDate
        ) && candidate.start >= start
            && date_end <= end
    })
}

pub(super) fn add_questions(questions: &mut BTreeMap<String, Value>, candidates: &[Candidate]) {
    for (index, passage) in candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == CandidateKind::ProseStatement)
    {
        questions.insert(format!("prose_role_{index}"), json!({
            "type":"choice",
            "instructions":format!("What kind of content does this exact acquired passage record? Classify its wording, without authenticating its claims. A planned occurrence remains planned; a negated or hypothetical occurrence is a statement about it, not an occurrence established by the source. Passage: {}", passage.quote),
            "criteria":{
                "event":"A distinct actual or explicitly scheduled occurrence is described, preserving its stated tense and qualification.",
                "statement":"An attributed assertion, observation, description or qualification; this includes negated and hypothetical occurrences.",
                "question":"An explicit source-stated question, without inferring an answer.",
                "decision":"An explicit source-stated choice or intended action, without asserting it was implemented."
            }
        }));
        for (date_index, date) in dates_in(passage, candidates) {
            questions.insert(format!("prose_date_{index}_{date_index}"), json!({
                "type":"noul",
                "instructions":format!("Does this exact passage assign the literal date `{}` to the occurrence it describes? A scheduled date is a date for that planned occurrence, never evidence that it already happened. Reject a document/publication date, a comparison date, a negated occurrence, or an unrelated date. Passage: {}", date.value, passage.quote),
                "criteria":{"true":"The passage explicitly assigns this date or date precision to its own actual or planned occurrence.","false":"The date is not the date of that occurrence, or no actual or planned occurrence is described."}
            }));
        }
    }
}

pub(super) fn append_knowledge(
    candidates: &[Candidate],
    accepted: &[&Candidate],
    answers: &serde_json::Map<String, Value>,
    model: &str,
    draft: &mut KnowledgeDraft,
) -> Result<(), ProviderError> {
    let mut used_labels = draft
        .entities
        .iter()
        .map(|entity| entity.label.to_lowercase())
        .collect::<std::collections::HashSet<_>>();
    for (index, passage) in candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == CandidateKind::ProseStatement)
    {
        let (role, probability) = validated_choice(
            answers,
            &format!("prose_role_{index}"),
            &["event", "statement", "question", "decision"],
        )?;
        // Validate every requested answer even when this passage was rejected.
        let dates = dates_in(passage, candidates)
            .map(|(date_index, date)| {
                Ok((
                    date,
                    validated_noul(answers, &format!("prose_date_{index}_{date_index}"))?,
                ))
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;
        if !accepted
            .iter()
            .any(|candidate| std::ptr::eq(*candidate, passage))
        {
            continue;
        }
        let kind = if probability >= SUPPORT_THRESHOLD {
            role.as_str()
        } else {
            "statement"
        };
        let mut label = passage.quote.chars().take(180).collect::<String>();
        if label.len() < passage.quote.len() {
            label.push('…');
        }
        // Labels bind facts to pages. Disambiguate only collisions, including
        // case variants that would otherwise hash to the same page identity.
        let base_label = label.clone();
        let mut ordinal = 2;
        while !used_labels.insert(label.to_lowercase()) {
            label = format!("{base_label} ({ordinal})");
            ordinal += 1;
        }
        let evidence = passage.evidence();
        draft.entities.push(EntityDraft {
            kind: kind.into(),
            label: label.clone(),
            evidence: evidence.clone(),
        });
        draft.facts.push(FactDraft {
            subject: label.clone(),
            property: "source_statement".into(),
            value: passage.quote.clone(),
            evidence: evidence.clone(),
            record_key: None,
        });
        draft.decisions.push(SemanticDecision {
            question: format!("prose_role:{}", passage.quote),
            model: model.into(),
            outcome: kind.into(),
            probability: Some(probability),
        });
        if kind == "event" {
            for (date, support) in dates {
                if support < SUPPORT_THRESHOLD
                    || !accepted
                        .iter()
                        .any(|candidate| std::ptr::eq(*candidate, date))
                {
                    continue;
                }
                let mut evidence = date.evidence();
                evidence.qualifier = Some(format!("As stated in the source: {}", passage.quote));
                draft.facts.push(FactDraft {
                    subject: label.clone(),
                    property: "event_date".into(),
                    value: date.value.clone(),
                    evidence,
                    record_key: None,
                });
                draft.decisions.push(SemanticDecision {
                    question: format!("prose_event_date:{}", date.value),
                    model: model.into(),
                    outcome: "supported".into(),
                    probability: Some(support),
                });
            }
        }
    }
    Ok(())
}
