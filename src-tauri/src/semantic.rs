//! Semantic work at the application boundary. Providers propose grounded drafts;
//! the collection validates evidence and owns identities and Markdown writes.
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeDraft {
    pub entities: Vec<EntityDraft>,
    pub facts: Vec<FactDraft>,
    pub relationships: Vec<RelationshipDraft>,
    #[serde(default)]
    pub tags: Vec<TagDraft>,
    #[serde(default)]
    pub decisions: Vec<SemanticDecision>,
    #[serde(default)]
    pub source_update: Option<SourceUpdateDraft>,
    #[serde(default)]
    pub correction_candidates: Vec<CorrectionCandidateDraft>,
    #[serde(default)]
    pub correction_alignments: Vec<CorrectionAlignmentDraft>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceUpdateRole {
    CompleteReplacement,
    Supplement,
    Conditional,
    TargetedCorrection,
    Unknown,
}

impl SourceUpdateRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CompleteReplacement => "complete_replacement",
            Self::Supplement => "supplement",
            Self::Conditional => "conditional",
            Self::TargetedCorrection => "targeted_correction",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceUpdateDraft {
    pub role: SourceUpdateRole,
    pub evidence: EvidenceDraft,
    pub certainty: f64,
    #[serde(default)]
    pub source_date: Option<String>,
    #[serde(default)]
    pub source_date_evidence: Option<EvidenceDraft>,
    #[serde(default)]
    pub source_date_certainty: f64,
    #[serde(default)]
    pub source_revision: Option<u64>,
    #[serde(default)]
    pub source_revision_evidence: Option<EvidenceDraft>,
    #[serde(default)]
    pub source_revision_certainty: f64,
}

#[derive(Debug, Clone)]
pub struct SemanticJob {
    pub source_id: String,
    pub source_version_id: String,
    pub attempt: u32,
    pub source_text: String,
    pub prior_source_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDecision {
    pub question: String,
    pub model: String,
    pub outcome: String,
    pub probability: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDraft {
    pub kind: String,
    pub label: String,
    pub evidence: EvidenceDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactDraft {
    pub subject: String,
    pub property: String,
    pub value: String,
    pub evidence: EvidenceDraft,
    #[serde(default)]
    pub record_key: Option<String>,
}

/// A deterministic candidate found in an explicit correction phrase. It is not
/// publishable until the application matches it to the current event field and
/// the provider independently aligns the incoming and prior evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionCandidateDraft {
    pub event_label: String,
    pub property: String,
    pub previous_value: String,
    pub corrected_value: String,
    pub evidence: EvidenceDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionAlignmentDraft {
    pub candidate: CorrectionCandidateDraft,
    pub outcome: FieldAlignmentOutcome,
    pub certainty: f64,
    pub model: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldAlignmentOutcome {
    SameFieldCorrection,
    ConditionalOrRejected,
    DifferentFieldOrEvent,
    Uncertain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipDraft {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub qualifier: Option<String>,
    pub evidence: EvidenceDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagDraft {
    pub subject: String,
    pub label: String,
    pub evidence: EvidenceDraft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceDraft {
    pub quote: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub origin: String,
    pub qualifier: Option<String>,
    #[serde(default)]
    pub offset_basis: Option<String>,
    #[serde(default)]
    pub source_location: Option<String>,
}

pub trait SemanticProvider: Send + Sync {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError>;

    /// Supplies the previous complete projection for source-version reconciliation.
    /// Providers that do not use comparative context remain source compatible.
    fn form_knowledge_with_prior(
        &self,
        source_text: &str,
        _prior_source_text: Option<&str>,
    ) -> std::result::Result<KnowledgeDraft, ProviderError> {
        self.form_knowledge(source_text)
    }
}

#[derive(Debug, Clone)]
pub struct ProviderError {
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn recoverable(message: String) -> Self {
        Self {
            message,
            retryable: true,
        }
    }

    pub fn permanent(message: String) -> Self {
        Self {
            message,
            retryable: false,
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ProviderError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePageSummary {
    pub page_id: String,
    pub title: String,
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePage {
    pub page_id: String,
    pub source_id: String,
    pub title: String,
    pub kind: String,
    pub markdown: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_edit_status: Option<String>,
}
