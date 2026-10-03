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
    pub decisions: Vec<SemanticDecision>,
}

#[derive(Debug, Clone)]
pub struct SemanticJob {
    pub source_id: String,
    pub source_version_id: String,
    pub attempt: u32,
    pub source_text: String,
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
pub struct EvidenceDraft {
    pub quote: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub origin: String,
    pub qualifier: Option<String>,
}

pub trait SemanticProvider: Send + Sync {
    fn form_knowledge(
        &self,
        source_text: &str,
    ) -> std::result::Result<KnowledgeDraft, ProviderError>;
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
}
