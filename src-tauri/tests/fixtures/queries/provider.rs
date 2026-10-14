use knowledge_garden::semantic::{
    EntityDraft, EvidenceDraft, FactDraft, KnowledgeDraft, RelationshipDraft, SemanticProvider,
};
pub const TEXT: &str = include_str!("field-notes.txt");
pub struct LabeledGraph;
fn evidence(quote: &str) -> EvidenceDraft {
    let byte_start = TEXT.find(quote).unwrap();
    EvidenceDraft {
        quote: quote.into(),
        byte_start,
        byte_end: byte_start + quote.len(),
        origin: "observed".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    }
}
impl SemanticProvider for LabeledGraph {
    fn form_knowledge(
        &self,
        _: &str,
    ) -> Result<KnowledgeDraft, knowledge_garden::semantic::ProviderError> {
        Ok(KnowledgeDraft {
            entities: [
                ("person", "Maya"),
                ("person", "Theo"),
                ("event", "V17"),
                ("event", "V18"),
                ("place", "Riverside"),
                ("place", "Paris"),
                ("place", "Riverside Park"),
            ]
            .into_iter()
            .map(|(kind, label)| EntityDraft {
                kind: kind.into(),
                label: label.into(),
                evidence: evidence(label),
            })
            .collect(),
            facts: [("V17", "2024-05-17"), ("V18", "2024-06-18")]
                .into_iter()
                .map(|(subject, value)| FactDraft {
                    subject: subject.into(),
                    property: "event_date".into(),
                    value: value.into(),
                    evidence: evidence(value),
                    record_key: None,
                })
                .collect(),
            relationships: [
                ("V17", "Maya", "observed_by", "Maya observed V17"),
                ("V18", "Maya", "observed_by", "Maya observed V18"),
                (
                    "V17",
                    "Riverside",
                    "occurred_at",
                    "V17 occurred at Riverside with location uncertainty",
                ),
                ("V18", "Paris", "occurred_at", "V18 occurred at Paris"),
                ("Riverside", "Maya", "links_to", "Riverside links to Maya"),
            ]
            .into_iter()
            .map(|(from, to, kind, quote)| RelationshipDraft {
                from: from.into(),
                to: to.into(),
                kind: kind.into(),
                qualifier: quote
                    .contains("uncertainty")
                    .then(|| "location uncertainty".into()),
                evidence: evidence(quote),
            })
            .collect(),
            ..KnowledgeDraft::default()
        })
    }
}
