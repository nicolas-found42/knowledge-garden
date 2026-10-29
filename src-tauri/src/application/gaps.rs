//! On-demand explanations of existing recovery and evidence states.
use super::{read_knowledge_header, Application, ExtractionState, Result, SourceInfo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub category: String,
    pub scope: String,
    pub detail: String,
    pub recovery: String,
    pub source_id: Option<String>,
    pub original_available: bool,
    pub evidence: Option<String>,
    pub original_version: Option<(String, String)>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapInspection {
    pub gaps: Vec<KnowledgeGap>,
    pub truncated: bool,
}
impl Application {
    pub fn inspect_gaps(&self, page_id: &str) -> Result<GapInspection> {
        let mut gaps = Vec::new();
        if page_id.starts_with("source-") {
            self.source_dir(page_id)?;
            self.source_gaps(page_id, &mut gaps);
        } else {
            let page = self.open_knowledge_page(page_id)?;
            let header = read_knowledge_header(&self.knowledge_path(page_id)?)?;
            self.source_gaps(&page.source_id, &mut gaps);
            if header.external_edit_status.as_deref() == Some("uncertain") {
                gaps.push(gap("insufficient_support", page_id, "External Markdown is preserved, but some edits could not safely become current facts.", "Automatic updates continue for unedited facts. Uninterpreted owner content remains readable.", None, false, None));
            }
            for relationship in &header.relationships {
                let original = relationship.supports.iter().rev().find_map(|support| {
                    let source = self.open_source(&support.source_id).ok()?;
                    let version = source
                        .info
                        .versions_seen
                        .iter()
                        .find(|v| v.source_version_id == support.source_version_id)?;
                    self.original_version_path(
                        &support.source_id,
                        &support.source_version_id,
                        &version.asset,
                    )
                    .ok()?;
                    Some((
                        support.source_id.clone(),
                        support.source_version_id.clone(),
                        version.asset.clone(),
                        support,
                    ))
                });
                let legacy_available = relationship.supports.is_empty()
                    && self.original_path(&header.source_id).is_ok();
                let available = original.is_some() || legacy_available;
                let qualifier = original
                    .as_ref()
                    .and_then(|o| o.3.qualifier.as_ref().or(o.3.evidence.qualifier.as_ref()))
                    .or(relationship.qualifier.as_ref())
                    .or(relationship.evidence.qualifier.as_ref());
                let evidence = original
                    .as_ref()
                    .map(|o| o.3.evidence.quote.clone())
                    .unwrap_or_else(|| relationship.evidence.quote.clone());
                if !available || qualifier.is_some() {
                    let mut explanation = gap("insufficient_support", &relationship.relationship_id,
                        &format!("{} → {}: {}", relationship.from_page_id, relationship.to_page_id, qualifier.map(String::as_str).unwrap_or("Retained supporting original unavailable.")),
                        "The relationship retains its qualification. Recovery cannot manufacture support; available source context remains readable.", Some(original.as_ref().map(|o| o.0.clone()).unwrap_or_else(|| header.source_id.clone())), available, Some(evidence));
                    explanation.original_version =
                        original.map(|(_, version, asset, _)| (version, asset));
                    gaps.push(explanation);
                }
                if gaps.len() >= 100 {
                    break;
                }
            }
        }
        let truncated = gaps.len() >= 100;
        gaps.truncate(100);
        Ok(GapInspection { gaps, truncated })
    }
    fn bind_extraction_gap(&self, info: &SourceInfo, explanation: &mut KnowledgeGap) {
        let version = info.extraction_version_id.as_deref().or_else(|| {
            if info
                .pending_version_id
                .as_deref()
                .is_some_and(|id| id != info.sha256)
            {
                None
            } else {
                Some(info.sha256.as_str())
            }
        });
        if let Some(version) = version {
            if let Some(record) = info
                .versions_seen
                .iter()
                .find(|v| v.source_version_id == version)
            {
                explanation.original_available = self
                    .original_version_path(&info.source_id, version, &record.asset)
                    .is_ok();
                explanation.original_version = Some((version.into(), record.asset.clone()));
                explanation.scope = format!("{version}: {}", explanation.scope);
            }
        } else {
            explanation.original_available = false;
            explanation.detail.push_str(" This older extraction record does not identify its version. Current and pending originals are separately available from source information; this explanation cannot choose one safely.");
        }
    }
    fn bind_pending_gap(&self, info: &SourceInfo, explanation: &mut KnowledgeGap) {
        if let Some(pending) = info.pending_version_id.as_deref() {
            let version = info
                .versions_seen
                .iter()
                .find(|v| v.source_version_id == pending);
            explanation.original_available = version.is_some_and(|v| {
                self.original_version_path(&info.source_id, pending, &v.asset)
                    .is_ok()
            });
            explanation.original_version = version.map(|v| (pending.into(), v.asset.clone()));
            if explanation.scope != pending {
                explanation.scope = format!("{pending}: {}", explanation.scope);
            }
        }
    }
    fn source_gaps(&self, id: &str, gaps: &mut Vec<KnowledgeGap>) {
        let available = self.original_path(id).is_ok();
        let Ok(page) = self.open_source(id) else {
            gaps.push(gap("source_unavailable", id, "The source page is unavailable. Existing knowledge has not been replaced with invented content.", "Automatic collection reconciliation continues; restoration requires the missing source material.", Some(id.into()), false, None));
            return;
        };
        if !available {
            gaps.push(gap("source_unavailable", id, "The retained original is unavailable; readable source and knowledge pages remain accessible.", "Automatic reconciliation continues. A missing original cannot be regenerated from knowledge.", Some(id.into()), false, None));
        }
        let info = &page.info;
        for linked in info.knowledge_pages.iter().take(100) {
            if let Err(error) = self.knowledge_path(&linked.page_id) {
                let detail = error.to_string();
                gaps.push(gap(if detail.contains("ambiguous") { "identity_ambiguous" } else { "insufficient_support" }, &linked.page_id, &detail, "Owner files remain preserved. Automatic reconciliation continues without choosing an ambiguous identity.", Some(id.into()), available, None));
            }
        }
        let recovery = recovery(info);
        if !matches!(
            info.extraction,
            ExtractionState::TextPreserved | ExtractionState::StructuredText
        ) {
            let mut explanation = gap(
                "extraction_incomplete",
                id,
                &info.extraction_detail,
                &recovery,
                Some(id.into()),
                available,
                None,
            );
            self.bind_extraction_gap(info, &mut explanation);
            gaps.push(explanation);
        }
        for part in info.extraction_coverage.as_deref().unwrap_or_default() {
            if !matches!(part.status, crate::office::CoverageStatus::Complete) {
                let mut explanation = gap(
                    "extraction_incomplete",
                    part.source_location.as_deref().unwrap_or(id),
                    &part.detail,
                    &recovery,
                    Some(id.into()),
                    available,
                    None,
                );
                self.bind_extraction_gap(info, &mut explanation);
                gaps.push(explanation);
            }
        }
        if info.update_status.is_some() {
            let category = update_category(info).unwrap_or("update_pending");
            let mut explanation = gap(category, info.pending_version_id.as_deref().unwrap_or(id), info.semantic_error.as_deref().unwrap_or("The replacement has not completed. Existing successful knowledge remains current."), &recovery, Some(id.into()), available, None);
            self.bind_pending_gap(info, &mut explanation);
            gaps.push(explanation);
        } else if info.semantic_state != "complete" {
            gaps.push(gap(if info.semantic_state == "failed" {"update_failed"} else if info.semantic_state == "unavailable" {"insufficient_support"} else {"update_pending"}, id, info.semantic_error.as_deref().unwrap_or("Semantic processing has not supplied supported current knowledge for this source."), &recovery, Some(id.into()), available, None));
        }
    }
}
fn recovery(info: &SourceInfo) -> String {
    if let Some(media) = info
        .pending_audio_processing
        .as_ref()
        .or(info.audio_processing.as_ref())
    {
        if matches!(media.state.as_str(), "pending" | "processing") {
            return "Automatic bounded local media processing is queued or running; semantic recovery follows usable extraction.".into();
        }
        if let Some(at) = media.retry_at_ms {
            return format!("Automatic local media retry is scheduled at epoch milliseconds {at}. Existing readable content remains available.");
        }
    }
    if let Some(at) = &info.semantic_retry_at {
        format!("Automatic semantic retry is scheduled at epoch milliseconds {at}. Existing readable content remains available.")
    } else if matches!(info.semantic_state.as_str(), "pending" | "processing") {
        "Automatic processing is queued or running; no owner approval is required.".into()
    } else {
        "No automatic retry is currently scheduled. Supported current content remains readable; unavailable capabilities are not treated as success.".into()
    }
}
fn gap(
    category: &str,
    scope: &str,
    detail: &str,
    recovery: &str,
    source_id: Option<String>,
    original_available: bool,
    evidence: Option<String>,
) -> KnowledgeGap {
    KnowledgeGap {
        category: category.into(),
        scope: scope.into(),
        detail: detail.into(),
        recovery: recovery.into(),
        source_id,
        original_available,
        evidence,
        original_version: None,
    }
}

pub(super) fn update_category(info: &SourceInfo) -> Option<&'static str> {
    info.update_status.as_deref().map(|status| {
        if status == "uncertain"
            && info
                .semantic_error
                .as_deref()
                .is_some_and(|s| s.contains("identity") || s.contains("ambiguous"))
        {
            "identity_ambiguous"
        } else if matches!(status, "failed" | "incomplete" | "uncertain") {
            "update_failed"
        } else {
            "update_pending"
        }
    })
}
