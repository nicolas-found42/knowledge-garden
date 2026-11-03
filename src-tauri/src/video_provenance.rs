//! Checks that video claims do not gain support from unrelated tracks or frames.
use crate::semantic::{EvidenceDraft, KnowledgeDraft};

fn item<'a>(source: &'a str, evidence: &EvidenceDraft) -> Option<&'a str> {
    let quote = source.get(evidence.byte_start..evidence.byte_end)?;
    if quote != evidence.quote || quote.is_empty() {
        return None;
    }
    let start = source
        .get(..evidence.byte_start)?
        .rfind('\n')
        .map_or(0, |i| i + 1);
    let end = source[evidence.byte_start..]
        .find('\n')
        .map_or(source.len(), |i| evidence.byte_start + i);
    if evidence.byte_end > end {
        return None;
    }
    let line = source.get(start..end)?;
    if !(line.starts_with("[AUDIO ") || line.starts_with("[VIDEO FRAME ")) {
        return None;
    }
    Some(line)
}

/// Called after the retained segment and frame timestamps have been validated.
pub fn validate(source: &str, draft: &KnowledgeDraft) -> Result<(), String> {
    for relation in &draft.relationships {
        let line = item(source, &relation.evidence)
            .ok_or("Video relationship evidence must refer to one retained item")?;
        // An explicit ordered phrase is a deliberately narrow proof of a stated relation.
        // This does not treat two objects or tracks appearing together as a relationship.
        let phrase = format!("{} {} {}", relation.from, relation.kind, relation.to).to_lowercase();
        let content = line.split_once("] ").map(|(_, text)| text).unwrap_or("");
        let statement = content.trim().trim_end_matches('.').trim();
        if relation.from.trim().is_empty()
            || relation.kind.trim().is_empty()
            || relation.to.trim().is_empty()
            || relation.evidence.quote.to_lowercase() != phrase
            || statement.to_lowercase() != phrase
            || line.contains("channel=generated_interpretation")
        {
            return Err(
                "Video relationship needs an explicit relation in one non-generated item".into(),
            );
        }
    }
    for fact in &draft.facts {
        let line = item(source, &fact.evidence)
            .ok_or("Video fact evidence must refer to one retained item")?;
        let quote = fact.evidence.quote.to_lowercase();
        if fact.value.trim().is_empty()
            || !quote.contains(&fact.value.to_lowercase())
            || (fact.subject != "Recorded video" && !quote.contains(&fact.subject.to_lowercase()))
        {
            return Err("Video facts require their subject and value in the same item".into());
        }
        if fact.subject != "Recorded video" {
            let statement = line
                .split_once("] ")
                .map(|(_, text)| text.trim().trim_end_matches('.').trim().to_lowercase())
                .unwrap_or_default();
            let explicit =
                format!("{} {} {}", fact.subject, fact.property, fact.value).to_lowercase();
            if statement != explicit || quote != explicit {
                return Err(
                    "Video facts about named subjects need an explicit statement in one item"
                        .into(),
                );
            }
        }
        if line.contains("Qwen interpretation:")
            && (fact.property != "video_frame_interpretation" || fact.value != fact.evidence.quote)
        {
            return Err(
                "Generated frame interpretations must remain exact, qualified interpretations"
                    .into(),
            );
        }
    }
    for evidence in draft
        .entities
        .iter()
        .map(|x| &x.evidence)
        .chain(draft.tags.iter().map(|x| &x.evidence))
    {
        item(source, evidence).ok_or("Video claim evidence must refer to one retained item")?;
    }
    Ok(())
}
