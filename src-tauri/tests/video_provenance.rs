use knowledge_garden::{
    semantic::{EvidenceDraft, FactDraft, KnowledgeDraft, RelationshipDraft},
    video_provenance::validate,
};
fn evidence(source: &str, quote: &str) -> EvidenceDraft {
    let start = source.find(quote).unwrap();
    EvidenceDraft {
        quote: quote.into(),
        byte_start: start,
        byte_end: start + quote.len(),
        origin: "generated_interpretation".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    }
}
#[test]
fn fruit_and_visits_do_not_establish_a_relationship() {
    let source = "[VIDEO FRAME 5000 ms; channel=generated_interpretation] Qwen interpretation: An orange appears.\n[AUDIO 0–4300 ms confidence=unavailable speaker=unidentified] There were 12 visits.\n";
    let mut draft = KnowledgeDraft::default();
    draft.relationships.push(RelationshipDraft {
        from: "orange".into(),
        kind: "has".into(),
        to: "12 visits".into(),
        qualifier: None,
        evidence: evidence(source, "There were 12 visits."),
    });
    assert!(validate(source, &draft).is_err());
    draft.relationships.clear();
    draft.facts.push(FactDraft {
        subject: "image".into(),
        property: "count".into(),
        value: "12".into(),
        evidence: evidence(source, "An orange appears."),
        record_key: None,
    });
    assert!(validate(source, &draft).is_err());
    draft.facts.clear();
    draft.facts.push(FactDraft {
        subject: "orange".into(),
        property: "visits".into(),
        value: "12".into(),
        evidence: evidence(source, "There were 12 visits."),
        record_key: None,
    });
    assert!(validate(source, &draft).is_err());
}
#[test]
fn explicit_relation_in_one_transcript_is_accepted() {
    let source =
        "[AUDIO 0–4300 ms confidence=unavailable speaker=unidentified] Alice manages Orchard.\n";
    let mut draft = KnowledgeDraft::default();
    draft.relationships.push(RelationshipDraft {
        from: "Alice".into(),
        kind: "manages".into(),
        to: "Orchard".into(),
        qualifier: None,
        evidence: evidence(source, "Alice manages Orchard"),
    });
    assert!(validate(source, &draft).is_ok());
}

#[test]
fn quoted_question_or_negation_is_not_an_asserted_relation() {
    for sentence in [
        "Alice manages Orchard?",
        "Alice manages Orchard. That is false.",
    ] {
        let source =
            format!("[AUDIO 0–4000 ms confidence=unavailable speaker=unidentified] {sentence}\n");
        let mut draft = KnowledgeDraft::default();
        draft.relationships.push(RelationshipDraft {
            from: "Alice".into(),
            kind: "manages".into(),
            to: "Orchard".into(),
            qualifier: None,
            evidence: evidence(&source, "Alice manages Orchard"),
        });
        assert!(validate(&source, &draft).is_err());
    }
}
