//! Explicit mixed-material routes. Contextual stops are checked against current retained material.
use super::{
    read_bounded_text,
    views::{self, ViewTarget},
    Application, GardenError, PageSearchRequest, Result, MAX_TEXT_BYTES,
};
use crate::semantic::{NavigationCandidate, NavigationJudgments, SemanticProvider};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;
const PAGE: usize = 50;
const MAX_QUOTE: usize = 2048;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TrailContext {
    Span {
        source_version_id: String,
        basis: String,
        byte_start: usize,
        byte_end: usize,
        quote: String,
    },
    Message {
        source_version_id: String,
        message_id: String,
    },
    Media {
        source_version_id: String,
        at_ms: u64,
        channel: String,
    },
}
impl TrailContext {
    pub fn version(&self) -> &str {
        match self {
            Self::Span {
                source_version_id, ..
            }
            | Self::Message {
                source_version_id, ..
            }
            | Self::Media {
                source_version_id, ..
            } => source_version_id,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrailStop {
    pub stop_id: String,
    pub label: String,
    pub target: ViewTarget,
    pub context: Option<TrailContext>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedStop {
    pub stop: TrailStop,
    pub status: String,
    pub title: String,
    pub detail: String,
    pub quote: Option<String>,
    pub locator: Option<String>,
    pub retained_asset: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TrailChoices {
    pub stops: Vec<TrailStop>,
    pub next_offset: Option<usize>,
    pub detail: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TrailSuggestions {
    pub usage: Option<crate::semantic::NavigationUsage>,
    pub stops: Vec<TrailStop>,
    pub judgments: Vec<f64>,
    pub model: Option<String>,
    pub detail: String,
}
pub struct SuggestionJob {
    provider: Arc<dyn SemanticProvider>,
    goal: String,
    candidates: Vec<NavigationCandidate>,
    stops: Vec<TrailStop>,
    fingerprints: Vec<String>,
}
pub struct EvaluatedSuggestions {
    stops: Vec<TrailStop>,
    fingerprints: Vec<String>,
    judgments: std::result::Result<NavigationJudgments, crate::semantic::ProviderError>,
}
impl SuggestionJob {
    pub fn evaluate(self) -> EvaluatedSuggestions {
        let judgments = self.provider.judge_navigation(&self.goal, &self.candidates);
        EvaluatedSuggestions {
            stops: self.stops,
            fingerprints: self.fingerprints,
            judgments,
        }
    }
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(GardenError::Invalid(message.into()))
}
fn version_id(id: &str) -> Result<()> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return invalid("Invalid trail source version.");
    }
    Ok(())
}
pub(super) fn validate_stops(stops: &[TrailStop]) -> Result<()> {
    if stops.len() > PAGE {
        return invalid("A trail supports at most 50 stops.");
    }
    let mut ids = std::collections::HashSet::new();
    for stop in stops {
        if Uuid::parse_str(&stop.stop_id).is_err()
            || !ids.insert(&stop.stop_id)
            || stop.label.trim().is_empty()
            || stop.label.len() > 512
        {
            return invalid("Trail stops need distinct identities and readable labels.");
        }
        if stop.target.source_id.is_some() == stop.target.page_id.is_some() {
            return invalid("A trail stop names exactly one source or page.");
        }
        if let Some(id) = &stop.target.source_id {
            views::source_id(id)?;
        }
        if let Some(id) = &stop.target.page_id {
            views::page_id(id)?;
        }
        if stop.target.title.as_ref().is_some_and(|v| v.len() > 512) {
            return invalid("Trail target title is too long.");
        }
        if let Some(context) = &stop.context {
            if stop.target.source_id.is_none() {
                return invalid("Source locations require a source identity.");
            }
            version_id(context.version())?;
            match context {
                TrailContext::Span {
                    basis,
                    byte_start,
                    byte_end,
                    quote,
                    ..
                } if !["retained_original_text", "readable_source_projection"]
                    .contains(&basis.as_str())
                    || byte_end <= byte_start
                    || byte_end - byte_start != quote.len()
                    || quote.is_empty()
                    || quote.len() > MAX_QUOTE =>
                {
                    return invalid("Invalid bounded trail passage.")
                }
                TrailContext::Media { channel, .. }
                    if !["audio", "video"].contains(&channel.as_str()) =>
                {
                    return invalid("A trail media location requires its audio or video origin.")
                }
                TrailContext::Message { message_id, .. }
                    if message_id.is_empty()
                        || message_id.len() > 512
                        || message_id.chars().any(char::is_control) =>
                {
                    return invalid("Invalid trail message identity.")
                }
                _ => {}
            }
        }
    }
    Ok(())
}
fn stop(target: ViewTarget, label: String, context: Option<TrailContext>) -> TrailStop {
    TrailStop {
        stop_id: Uuid::new_v4().to_string(),
        label,
        target,
        context,
    }
}
fn bounded(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}
fn message_rows(body: &str) -> Vec<(String, String)> {
    body.lines()
        .filter(|line| line.starts_with("[CONVERSATION session="))
        .filter_map(|line| {
            let id = line.split("; message=").nth(1)?.split(';').next()?;
            let id = percent_encoding::percent_decode_str(id)
                .decode_utf8()
                .ok()?
                .into_owned();
            Some((id, bounded(line, MAX_QUOTE)))
        })
        .collect()
}
fn superseded(source: &super::SourcePage) -> bool {
    let current = source
        .info
        .current_version_id
        .as_ref()
        .unwrap_or(&source.info.sha256);
    source
        .info
        .versions_seen
        .iter()
        .any(|v| &v.source_version_id == current && v.state == "superseded")
}
impl Application {
    fn trail_projection(&self, source: &super::SourcePage, basis: &str) -> Result<String> {
        if basis == "retained_original_text" {
            return read_bounded_text(
                &self.original_path(&source.info.source_id)?,
                MAX_TEXT_BYTES as u64,
            );
        }
        if !matches!(
            source.info.extraction,
            super::ExtractionState::StructuredText | super::ExtractionState::PartialText
        ) {
            return invalid(
                "This source has no extracted textual location. Its original remains available.",
            );
        }
        let current = source
            .info
            .current_version_id
            .as_ref()
            .unwrap_or(&source.info.sha256);
        if source
            .info
            .extraction_version_id
            .as_ref()
            .is_some_and(|owner| owner != current)
        {
            return invalid("Extraction belongs to a pending version; a published contextual location is unavailable.");
        }
        Ok(source.body.clone())
    }
    pub fn trail_source_choices(&self, source_id: &str, offset: usize) -> Result<TrailChoices> {
        views::source_id(source_id)?;
        let source = self.open_source(source_id)?;
        if superseded(&source) {
            return invalid("This source version is superseded. Select current material for a new trail location.");
        }
        self.original_path(source_id)?;
        let version = source
            .info
            .current_version_id
            .as_ref()
            .unwrap_or(&source.info.sha256)
            .clone();
        let target = ViewTarget {
            source_id: Some(source_id.into()),
            page_id: None,
            title: Some(source.info.title.clone()),
        };
        let mut stops = Vec::new();
        if crate::conversation::is_conversation(&source.info.format) {
            let body = self.trail_projection(&source, "readable_source_projection")?;
            for (id, _) in message_rows(&body) {
                stops.push(stop(
                    target.clone(),
                    format!("{} · message {}", source.info.title, bounded(&id, 120)),
                    Some(TrailContext::Message {
                        source_version_id: version.clone(),
                        message_id: id,
                    }),
                ));
            }
        }
        if let Some(media) = &source.info.audio_processing {
            if media.source_version_id.as_deref() == Some(&version) {
                for segment in &media.segments {
                    stops.push(stop(
                        target.clone(),
                        format!("{} · {} ms", source.info.title, segment.start_ms),
                        Some(TrailContext::Media {
                            source_version_id: version.clone(),
                            at_ms: segment.start_ms,
                            channel: "audio".into(),
                        }),
                    ));
                }
                for frame in &media.frames {
                    stops.push(stop(
                        target.clone(),
                        format!("{} · frame {} ms", source.info.title, frame.time_ms),
                        Some(TrailContext::Media {
                            source_version_id: version.clone(),
                            at_ms: frame.time_ms,
                            channel: "video".into(),
                        }),
                    ));
                }
            }
        }
        let total = stops.len();
        Ok(TrailChoices {stops:stops.into_iter().skip(offset).take(PAGE).collect(),next_offset:(offset.saturating_add(PAGE)<total).then_some(offset.saturating_add(PAGE)),detail:"Locations come from existing messages and decoded media evidence. Select source wording to add a passage; no stop is saved until you explicitly save the trail.".into()})
    }
    pub fn trail_selected_passage(&self, source_id: &str, quote: &str) -> Result<TrailStop> {
        views::source_id(source_id)?;
        if quote.trim().is_empty() || quote.len() > MAX_QUOTE {
            return invalid("Select 1–2048 bytes of source wording for a trail passage.");
        }
        let source = self.open_source(source_id)?;
        if superseded(&source) {
            return invalid("This source version is superseded. Select current material for a new trail location.");
        }
        let basis = if matches!(
            source.info.extraction,
            super::ExtractionState::TextPreserved
        ) {
            "retained_original_text"
        } else {
            "readable_source_projection"
        };
        let text = self.trail_projection(&source, basis)?;
        let hits = text
            .match_indices(quote)
            .take(2)
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if hits.len() != 1 {
            return invalid("The selected wording is missing or repeated; select a unique passage with more surrounding context. No location was guessed.");
        }
        let version = source
            .info
            .current_version_id
            .as_ref()
            .unwrap_or(&source.info.sha256)
            .clone();
        Ok(stop(
            ViewTarget {
                source_id: Some(source_id.into()),
                page_id: None,
                title: Some(source.info.title.clone()),
            },
            format!("{} · passage", source.info.title),
            Some(TrailContext::Span {
                source_version_id: version,
                basis: basis.into(),
                byte_start: hits[0],
                byte_end: hits[0] + quote.len(),
                quote: quote.into(),
            }),
        ))
    }
    pub fn resolve_trail(&self, stops: Vec<TrailStop>) -> Result<Vec<ResolvedStop>> {
        validate_stops(&stops)?;
        Ok(stops
            .into_iter()
            .map(|stop| self.resolve_trail_stop(stop))
            .collect())
    }
    fn resolve_trail_stop(&self, stop: TrailStop) -> ResolvedStop {
        let mut resolved = ResolvedStop {
            title: stop
                .target
                .title
                .clone()
                .unwrap_or_else(|| stop.label.clone()),
            stop,
            status: "unavailable".into(),
            detail: "This stop is unavailable; its identity and position are preserved.".into(),
            quote: None,
            locator: None,
            retained_asset: None,
        };
        let target = &resolved.stop.target;
        let Some(id) = &target.source_id else {
            if let Some(page) = target
                .page_id
                .as_ref()
                .and_then(|id| self.open_knowledge_page(id).ok())
            {
                resolved.title = page.title;
                resolved.status = "available".into();
                resolved.detail = "Current page resolved by its stable identity.".into();
            }
            return resolved;
        };
        let Ok(source) = self.open_source(id) else {
            return resolved;
        };
        resolved.title = source.info.title.clone();
        if superseded(&source) {
            resolved.detail="This source version is superseded. The saved stop remains in place, but obsolete source wording has not been replayed or replaced with another source.".into();
            return resolved;
        }
        if self.original_path(id).is_err() {
            return resolved;
        }
        let Some(context) = &resolved.stop.context else {
            resolved.status = "available".into();
            resolved.detail = "Current source page resolved by its stable identity.".into();
            return resolved;
        };
        resolved.retained_asset = source
            .info
            .versions_seen
            .iter()
            .find(|v| v.source_version_id == context.version())
            .and_then(|v| {
                self.original_version_path(id, &v.source_version_id, &v.asset)
                    .ok()
                    .map(|_| v.asset.clone())
            });
        let current = source
            .info
            .current_version_id
            .as_ref()
            .unwrap_or(&source.info.sha256);
        if context.version() != current {
            resolved.status = "changed".into();
            resolved.detail="The source changed. This saved location belongs to an earlier version; no different passage or obsolete knowledge has been substituted. Open current source separately or inspect the exact retained original.".into();
            return resolved;
        }
        match context {
            TrailContext::Span {
                basis,
                byte_start,
                byte_end,
                quote,
                ..
            } => {
                if let Ok(text) = self.trail_projection(&source, basis) {
                    if text.get(*byte_start..*byte_end) == Some(quote) {
                        resolved.quote = Some(quote.clone());
                        resolved.locator = Some(format!(
                            "{} · bytes {byte_start}–{byte_end}",
                            if basis == "retained_original_text" {
                                "Original source text"
                            } else {
                                "Readable source projection"
                            }
                        ));
                    }
                }
            }
            TrailContext::Message { message_id, .. } => {
                if let Ok(body) = self.trail_projection(&source, "readable_source_projection") {
                    let hits = message_rows(&body)
                        .into_iter()
                        .filter(|(id, _)| id == message_id)
                        .collect::<Vec<_>>();
                    if hits.len() == 1 {
                        resolved.quote = Some(hits[0].1.clone());
                        resolved.locator = Some(format!("Message {message_id}"));
                    }
                }
            }
            TrailContext::Media { at_ms, channel, .. } => {
                if let Some(media) = source
                    .info
                    .audio_processing
                    .as_ref()
                    .filter(|m| m.source_version_id.as_deref() == Some(current))
                {
                    if channel == "audio" {
                        if let Some(segment) = media
                            .segments
                            .iter()
                            .find(|s| s.start_ms <= *at_ms && *at_ms <= s.end_ms)
                        {
                            resolved.quote = Some(bounded(&segment.text, MAX_QUOTE));
                        }
                    }
                    if channel == "video" {
                        if let Some(frame) = media.frames.iter().find(|f| f.time_ms == *at_ms) {
                            resolved.quote = Some(bounded(
                                &frame
                                    .ocr
                                    .iter()
                                    .map(|row| row.text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                MAX_QUOTE,
                            ));
                        }
                    }
                    if resolved.quote.is_some() {
                        resolved.locator=Some(format!("{channel} media location {at_ms} ms; machine evidence retains its qualifications. Precise playback may require seeking in the retained original."));
                    }
                }
            }
        }
        if resolved.locator.is_some() {
            resolved.status = "available".into();
            resolved.detail =
                "The exact contextual location is available in current retained source material."
                    .into();
        } else {
            resolved.detail="The saved contextual location is unavailable or was externally changed. No similar passage was substituted.".into();
        }
        resolved
    }
    pub fn prepare_trail_suggestions(&mut self, goal: &str) -> Result<SuggestionJob> {
        if goal.trim().is_empty() || goal.len() > 1024 {
            return invalid("Describe the reading goal in 1–1024 bytes.");
        }
        let pages = self.search_pages(PageSearchRequest {
            query: goal.into(),
            ..Default::default()
        })?;
        let mut candidates = Vec::new();
        let mut stops = Vec::new();
        for page in pages.pages.into_iter().take(12) {
            candidates.push(NavigationCandidate {
                label: page.title.clone(),
                excerpt: bounded(&page.excerpt, 1024),
            });
            let target = if page.page_type == "source" {
                ViewTarget {
                    source_id: Some(page.source_id),
                    page_id: None,
                    title: Some(page.title.clone()),
                }
            } else {
                ViewTarget {
                    source_id: None,
                    page_id: Some(page.page_id),
                    title: Some(page.title.clone()),
                }
            };
            stops.push(stop(target, page.title, None));
        }
        let fingerprints = stops
            .iter()
            .map(|stop| self.trail_target_fingerprint(&stop.target))
            .collect::<Result<Vec<_>>>()?;
        Ok(SuggestionJob {
            fingerprints,
            provider: Arc::clone(&self.semantic_provider),
            goal: goal.into(),
            candidates,
            stops,
        })
    }
    fn trail_target_fingerprint(&self, target: &ViewTarget) -> Result<String> {
        let text = if let Some(id) = &target.source_id {
            let p = self.open_source(id)?;
            format!(
                "{}\n{:?}\n{}\n{}",
                p.info.sha256, p.info.current_version_id, p.info.title, p.body
            )
        } else {
            self.open_knowledge_page(
                target
                    .page_id
                    .as_deref()
                    .ok_or_else(|| GardenError::Invalid("Missing suggestion target.".into()))?,
            )?
            .markdown
        };
        Ok(hex::encode(Sha256::digest(text.as_bytes())))
    }
    pub fn finish_trail_suggestions(
        &self,
        evaluated: EvaluatedSuggestions,
    ) -> Result<TrailSuggestions> {
        let EvaluatedSuggestions {
            stops,
            fingerprints,
            judgments,
        } = evaluated;
        let judgments=match judgments {Ok(value)=>value,Err(error)=>return Ok(TrailSuggestions{stops:vec![],judgments:vec![],model:None,usage:None,detail:format!("Jev suggestions are unavailable: {error}. Existing trails remain usable; no alternative provider or invented stops were used.")})};
        if judgments.probabilities.len() != stops.len()
            || judgments
                .probabilities
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return invalid("Jev returned invalid navigation judgments; no stops were added.");
        }
        let current = self.resolve_trail(stops)?;
        let mut selected = current
            .into_iter()
            .zip(judgments.probabilities)
            .zip(fingerprints)
            .filter(|((r, p), fingerprint)| {
                r.status == "available"
                    && *p >= 0.7
                    && self
                        .trail_target_fingerprint(&r.stop.target)
                        .is_ok_and(|current| &current == fingerprint)
            })
            .map(|((r, p), _)| (r.stop, p))
            .collect::<Vec<_>>();
        selected.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.label.cmp(&b.0.label)));
        selected.truncate(5);
        let (stops, probabilities) = selected.into_iter().unzip();
        Ok(TrailSuggestions{stops,judgments:probabilities,model:(!judgments.model.is_empty()).then_some(judgments.model),usage:judgments.usage,detail:"Jev selected existing reading stops for this goal. Relevance estimates are suggestions, not evidence of a new relationship or a generated answer. Add chosen stops to your transient route; Save is explicit.".into()})
    }
}
