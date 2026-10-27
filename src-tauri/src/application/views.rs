//! Explicitly saved criteria; result sets and transient query cursors are never durable views.
use super::{
    boolean,
    collection::CollectionAxis,
    publication::{commit_publication_with_removals, validate_real_collection_ancestors},
    read_bounded_text, Application, GardenError, PageSearchRequest, Result,
};
use crate::timeline::TimelineRequest;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
const MAX_VIEW_BYTES: u64 = 256 * 1024;
const PAGE: usize = 50;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedSurface {
    Collection,
    Search,
    Query,
    Timeline,
    Paths,
    Comparison,
    Source,
    Knowledge,
    Trail,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewPresentation {
    #[default]
    Reading,
    Research,
    Canvas,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedGroup {
    pub group_id: String,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionCriteria {
    pub axis: CollectionAxis,
    pub query: String,
    pub presentation: ViewPresentation,
    pub expanded_groups: Vec<SavedGroup>,
    pub selected: Option<ViewTarget>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathCriteria {
    pub start_page_id: String,
    pub start_title: String,
    pub max_hops: usize,
    pub selected_target_page_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCriteria {
    pub source_id: String,
    pub title: String,
    pub line_start: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub source_id: Option<String>,
    pub page_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewDefinition {
    pub active: SavedSurface,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trail: Vec<super::trails::TrailStop>,
    pub search: Option<PageSearchRequest>,
    pub query: Option<boolean::BooleanQueryRequest>,
    pub collection: Option<CollectionCriteria>,
    pub timeline: Option<TimelineRequest>,
    pub paths: Option<PathCriteria>,
    #[serde(default)]
    pub comparison: Vec<ComparisonCriteria>,
    pub target: Option<ViewTarget>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredView {
    schema: u32,
    view_id: String,
    name: String,
    definition: ViewDefinition,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_yaml_ng::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedView {
    pub view_id: String,
    pub name: String,
    pub definition: ViewDefinition,
    pub warnings: Vec<String>,
    pub resolved_groups: Vec<SavedGroup>,
    pub resolved_references: Vec<super::collection::CollectionItem>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedViewSummary {
    pub view_id: String,
    pub name: String,
    pub path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDiagnostic {
    pub path: String,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedViews {
    pub views: Vec<SavedViewSummary>,
    pub next_offset: Option<usize>,
    pub diagnostics: Vec<ViewDiagnostic>,
}
struct Catalog {
    rows: Vec<(PathBuf, SavedViewSummary)>,
    diagnostics: Vec<ViewDiagnostic>,
}
impl ViewDefinition {
    fn validate(&self) -> Result<()> {
        let missing = match self.active {
            SavedSurface::Trail => self.trail.is_empty(),
            SavedSurface::Search => self.search.is_none(),
            SavedSurface::Query => self.query.is_none(),
            SavedSurface::Collection => self.collection.is_none(),
            SavedSurface::Timeline => self.timeline.is_none(),
            SavedSurface::Paths => self.paths.is_none(),
            SavedSurface::Comparison => self.comparison.len() != 2,
            SavedSurface::Source => self
                .target
                .as_ref()
                .and_then(|t| t.source_id.as_ref())
                .is_none(),
            SavedSurface::Knowledge => self
                .target
                .as_ref()
                .and_then(|t| t.page_id.as_ref())
                .is_none(),
        };
        super::trails::validate_stops(&self.trail)?;
        if missing {
            return invalid("The saved view is missing its active criteria; it cannot be broadened to a different view.");
        }
        if let Some(search) = &self.search {
            validate_filters(
                &search.query,
                &search.tags,
                search.date_from.as_deref(),
                search.date_to.as_deref(),
                search.format.as_deref(),
                search.processing_status.as_deref(),
            )?;
        }
        if let Some(query) = &self.query {
            boolean::saved_query_targets(query)?;
        }
        if let Some(collection) = &self.collection {
            if collection.query.len() > 512 || collection.expanded_groups.len() > 50 {
                return invalid("Saved collection scope is too large.");
            }
            for group in &collection.expanded_groups {
                if group.label.len() > 512 || group.group_id.len() > 1024 {
                    return invalid("Saved group is too large.");
                }
                match group.group_id.split_once(':') {
                    Some(("subject", id)) => page_id(id)?,
                    Some(("source", id)) => source_id(id)?,
                    Some(("tag", tag)) => {
                        super::normalize_tag(tag)?;
                    }
                    Some(("date", date)) => {
                        if !date.is_empty() {
                            super::validate_iso_date(date)?;
                        }
                    }
                    Some(("format", format)) => {
                        if format.len() > 32 {
                            return invalid("Saved format group is too large.");
                        }
                    }
                    None if group.group_id == "unclassified" => {}
                    _ => return invalid("Unknown saved collection group."),
                }
            }
        }
        if let Some(timeline) = &self.timeline {
            validate_filters(&timeline.query, &timeline.tags, None, None, None, None)?;
        }
        if let Some(paths) = &self.paths {
            page_id(&paths.start_page_id)?;
            if !(1..=32).contains(&paths.max_hops) || paths.start_title.len() > 512 {
                return invalid("Invalid saved relationship scope.");
            }
        }
        if self.comparison.len() > 2 {
            return invalid("A comparison supports two saved selections.");
        }
        for selection in &self.comparison {
            source_id(&selection.source_id)?;
            if selection.line_start == 0 || selection.title.len() > 512 {
                return invalid("Invalid saved comparison selection.");
            }
        }
        for target in self
            .target
            .iter()
            .chain(self.collection.iter().filter_map(|c| c.selected.as_ref()))
        {
            if target.title.as_ref().is_some_and(|t| t.len() > 512) {
                return invalid("Saved target title is too long.");
            }
            if target.source_id.is_some() == target.page_id.is_some() {
                return invalid("A saved target must name exactly one source or knowledge page.");
            }
        }
        for (kind, id) in self.references()? {
            if kind {
                source_id(&id)?;
            } else {
                page_id(&id)?;
            }
        }
        Ok(())
    }
    fn references(&self) -> Result<Vec<(bool, String)>> {
        let mut refs = Vec::new();
        for stop in &self.trail {
            if let Some(id) = &stop.target.source_id {
                refs.push((true, id.clone()));
            }
            if let Some(id) = &stop.target.page_id {
                refs.push((false, id.clone()));
            }
        }
        if let Some(query) = &self.query {
            refs.extend(
                boolean::saved_query_targets(query)?
                    .into_iter()
                    .map(|id| (false, id)),
            );
        }
        if let Some(paths) = &self.paths {
            refs.push((false, paths.start_page_id.clone()));
            if let Some(id) = &paths.selected_target_page_id {
                refs.push((false, id.clone()));
            }
        }
        if let Some(collection) = &self.collection {
            if let Some(target) = &collection.selected {
                if let Some(id) = &target.source_id {
                    refs.push((true, id.clone()));
                }
                if let Some(id) = &target.page_id {
                    refs.push((false, id.clone()));
                }
            }
            for group in &collection.expanded_groups {
                if let Some(id) = group.group_id.strip_prefix("subject:") {
                    refs.push((false, id.into()));
                } else if let Some(id) = group.group_id.strip_prefix("source:") {
                    refs.push((true, id.into()));
                }
            }
        }
        refs.extend(self.comparison.iter().map(|s| (true, s.source_id.clone())));
        if let Some(target) = &self.target {
            if let Some(id) = &target.source_id {
                refs.push((true, id.clone()));
            }
            if let Some(id) = &target.page_id {
                refs.push((false, id.clone()));
            }
        }
        refs.sort();
        refs.dedup();
        Ok(refs)
    }
    fn reset_positions(&mut self) {
        if let Some(search) = &mut self.search {
            search.offset = 0;
        }
        if let Some(timeline) = &mut self.timeline {
            timeline.offset = 0;
        }
    }
}
fn invalid<T>(message: &str) -> Result<T> {
    Err(GardenError::Invalid(message.into()))
}
pub(super) fn source_id(id: &str) -> Result<()> {
    if id.strip_prefix("source-").is_none_or(|v| {
        v.len() != 64
            || !v
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }) {
        return invalid("Invalid saved source reference.");
    }
    Ok(())
}
pub(super) fn page_id(id: &str) -> Result<()> {
    if !id.starts_with("page-")
        || id.len() > 128
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return invalid("Invalid saved page reference.");
    }
    Ok(())
}
fn view_id(id: &str) -> Result<()> {
    if id
        .strip_prefix("view-")
        .and_then(|v| Uuid::parse_str(v).ok())
        .is_none()
    {
        return invalid("Invalid saved view identity.");
    }
    Ok(())
}
fn view_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return invalid("A view name needs 1–120 readable characters.");
    }
    Ok(name.into())
}
fn validate_filters(
    query: &str,
    tags: &[String],
    from: Option<&str>,
    to: Option<&str>,
    format: Option<&str>,
    status: Option<&str>,
) -> Result<()> {
    if query.len() > 1024 || tags.len() > 32 || format.is_some_and(|f| f.is_empty() || f.len() > 32)
    {
        return invalid("Saved filters are too large or contain an empty format.");
    }
    for tag in tags {
        super::normalize_tag(tag)?;
    }
    let constraint = boolean::QueryConstraint {
        date_from: from.map(String::from),
        date_to: to.map(String::from),
        format: format.map(String::from),
        processing_status: status.map(String::from),
    };
    boolean::validate_saved_constraint(&constraint)
}
fn parse_view(text: &str) -> Result<(StoredView, String)> {
    let (header, body) = text
        .strip_prefix("---\n")
        .and_then(|t| t.split_once("\n---\n"))
        .ok_or_else(|| {
            GardenError::Invalid("Saved view has no readable YAML definition.".into())
        })?;
    let record: StoredView = serde_yaml_ng::from_str(header)?;
    if record.schema != 1 {
        return invalid("Saved view schema is unavailable; its criteria have not been replaced.");
    }
    view_id(&record.view_id)?;
    view_name(&record.name)?;
    record.definition.validate()?;
    Ok((record, body.into()))
}
fn serialize_view(record: &StoredView, body: &str) -> Result<Vec<u8>> {
    let text = format!("---\n{}---\n{}", serde_yaml_ng::to_string(record)?, body);
    if text.len() as u64 > MAX_VIEW_BYTES {
        return invalid("Saved view definition exceeds 256 KiB.");
    }
    Ok(text.into_bytes())
}
impl Application {
    fn view_catalog(&self) -> Result<Catalog> {
        let directory = self.root.join("views");
        let mut catalog = Catalog {
            rows: Vec::new(),
            diagnostics: Vec::new(),
        };
        if !directory.exists() {
            return Ok(catalog);
        }
        validate_real_collection_ancestors(&self.root, &directory.join(".catalog"))?;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            let loaded = (|| {
                validate_real_collection_ancestors(&self.root, &path)?;
                if !fs::symlink_metadata(&path)?.file_type().is_file() {
                    return invalid("Saved view Markdown must be an ordinary file, not a symlink.");
                }
                let text = read_bounded_text(&path, MAX_VIEW_BYTES)?;
                parse_view(&text)
            })();
            match loaded {
                Ok((record, _)) => catalog.rows.push((
                    path.clone(),
                    SavedViewSummary {
                        view_id: record.view_id,
                        name: record.name,
                        path: relative(&self.root, &path),
                    },
                )),
                Err(error) => catalog.diagnostics.push(ViewDiagnostic {
                    path: relative(&self.root, &path),
                    detail: error.to_string(),
                }),
            }
        }
        catalog.rows.sort_by(|a, b| {
            a.1.name
                .to_lowercase()
                .cmp(&b.1.name.to_lowercase())
                .then(a.1.view_id.cmp(&b.1.view_id))
                .then(a.0.cmp(&b.0))
        });
        let mut seen = HashSet::new();
        for (path, record) in &catalog.rows {
            if !seen.insert(record.view_id.clone()) {
                catalog.diagnostics.push(ViewDiagnostic{path:relative(&self.root,path),detail:format!("Saved view identity {} is duplicated; no owner file will be chosen automatically.",record.view_id)});
            }
        }
        Ok(catalog)
    }
    fn read_saved_view(&self, id: &str) -> Result<(PathBuf, StoredView, String, Vec<u8>)> {
        view_id(id)?;
        let catalog = self.view_catalog()?;
        let hits = catalog
            .rows
            .iter()
            .filter(|(_, r)| r.view_id == id)
            .collect::<Vec<_>>();
        if hits.len() != 1 {
            return invalid("The saved view is missing or its identity is ambiguous. Preserved Markdown has not been replaced.");
        }
        let path = hits[0].0.clone();
        validate_real_collection_ancestors(&self.root, &path)?;
        if !fs::symlink_metadata(&path)?.file_type().is_file() {
            return invalid("Saved view Markdown must be an ordinary file, not a symlink.");
        }
        let text = read_bounded_text(&path, MAX_VIEW_BYTES)?;
        let (record, body) = parse_view(&text)?;
        if record.view_id != id {
            return invalid("Saved view changed while opening; retry its preserved definition.");
        }
        Ok((path, record, body, text.into_bytes()))
    }
    pub fn list_saved_views(&self, query: &str, offset: usize) -> Result<SavedViews> {
        if query.len() > 512 {
            return invalid("Saved view name filter is too long.");
        }
        let mut catalog = self.view_catalog()?;
        catalog
            .rows
            .retain(|(_, r)| r.name.to_lowercase().contains(&query.to_lowercase()));
        let total = catalog.rows.len();
        Ok(SavedViews {
            views: catalog
                .rows
                .into_iter()
                .skip(offset)
                .take(PAGE)
                .map(|(_, r)| r)
                .collect(),
            next_offset: (offset.saturating_add(PAGE) < total)
                .then_some(offset.saturating_add(PAGE)),
            diagnostics: catalog.diagnostics,
        })
    }
    pub fn open_saved_view(&self, id: &str) -> Result<SavedView> {
        let (_, record, _, _) = self.read_saved_view(id)?;
        self.saved_view_dto(record)
    }
    fn saved_view_dto(&self, record: StoredView) -> Result<SavedView> {
        let mut warnings = Vec::new();
        let mut resolved_references = Vec::new();
        for (source, id) in record.definition.references()? {
            let resolved = if source {
                self.open_source(&id)
                    .ok()
                    .map(|page| super::collection::CollectionItem {
                        page_id: page.info.page_id.clone(),
                        source_id: page.info.source_id.clone(),
                        page_type: "source".into(),
                        title: page.info.title.clone(),
                        kind: "source".into(),
                        format: page.info.format.clone(),
                        processing_status: if self.original_path(&id).is_err() {
                            "source_unavailable".into()
                        } else {
                            super::processing_status(&page.info)
                        },
                    })
            } else {
                self.open_knowledge_page(&id).ok().map(|page|{
                    let metadata=self.index.query_row("SELECT format,processing_status FROM query_page_metadata WHERE page_id=?1",[&id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).unwrap_or_else(|_|(String::new(),"unavailable".into()));
                    super::collection::CollectionItem {page_id:page.page_id,source_id:page.source_id,page_type:"knowledge".into(),title:page.title,kind:page.kind,format:metadata.0,processing_status:metadata.1}
                })
            };
            if let Some(reference) = resolved {
                resolved_references.push(reference);
            } else {
                warnings.push(format!(
                    "Saved reference {id} is unavailable. Its scope has not been broadened."
                ));
            }
        }
        for target in record
            .definition
            .target
            .iter()
            .chain(
                record
                    .definition
                    .collection
                    .iter()
                    .filter_map(|c| c.selected.as_ref()),
            )
            .chain(record.definition.trail.iter().map(|s| &s.target))
        {
            if let Some(old_title) = &target.title {
                if let Some(current) = resolved_references.iter().find(|r| {
                    target
                        .source_id
                        .as_ref()
                        .is_some_and(|id| r.page_type == "source" && &r.source_id == id)
                        || target.page_id.as_ref() == Some(&r.page_id)
                }) {
                    if &current.title != old_title {
                        warnings.push(format!(
                            "Saved target ‘{old_title}’ now reads ‘{}’; its identity is preserved.",
                            current.title
                        ));
                    }
                }
            }
        }
        for selection in &record.definition.comparison {
            if let Some(current) = resolved_references
                .iter()
                .find(|r| r.page_type == "source" && r.source_id == selection.source_id)
            {
                if current.title != selection.title {
                    warnings.push(format!(
                        "Saved source ‘{}’ now reads ‘{}’; its identity is preserved.",
                        selection.title, current.title
                    ));
                }
            }
        }
        if let Some(paths) = &record.definition.paths {
            if let Some(current) = resolved_references
                .iter()
                .find(|r| r.page_id == paths.start_page_id)
            {
                if current.title != paths.start_title {
                    warnings.push(format!(
                        "Saved relationship start ‘{}’ now reads ‘{}’; its scope is preserved.",
                        paths.start_title, current.title
                    ));
                }
            }
        }
        let mut resolved_groups = Vec::new();
        if let Some(collection) = &record.definition.collection {
            for group in &collection.expanded_groups {
                let label = if let Some(id) = group.group_id.strip_prefix("subject:") {
                    self.open_knowledge_page(id).ok().map(|p| p.title)
                } else if let Some(id) = group.group_id.strip_prefix("source:") {
                    self.open_source(id)
                        .ok()
                        .map(|p| format!("Source neighborhood · {}", p.info.title))
                } else if let Some(tag) = group.group_id.strip_prefix("tag:") {
                    self.index
                        .query_row(
                            "SELECT MIN(label) FROM page_tags WHERE normalized=?1",
                            [tag],
                            |r| r.get::<_, Option<String>>(0),
                        )
                        .ok()
                        .flatten()
                } else {
                    Some(group.label.clone())
                };
                if let Some(label) = label {
                    if label != group.label {
                        warnings.push(format!(
                            "Saved group ‘{}’ now reads ‘{label}’; its stable scope is preserved.",
                            group.label
                        ));
                    }
                    resolved_groups.push(SavedGroup {
                        group_id: group.group_id.clone(),
                        label,
                    });
                } else {
                    warnings.push(format!(
                        "Saved group ‘{}’ is unavailable; its scope has not been replaced.",
                        group.label
                    ));
                }
            }
        }
        Ok(SavedView {
            view_id: record.view_id,
            name: record.name,
            definition: record.definition,
            warnings,
            resolved_groups,
            resolved_references,
        })
    }
    pub fn create_saved_view(
        &mut self,
        name: &str,
        mut definition: ViewDefinition,
    ) -> Result<SavedView> {
        definition.validate()?;
        definition.reset_positions();
        let name = view_name(name)?;
        if self
            .view_catalog()?
            .rows
            .iter()
            .any(|(_, r)| r.name.to_lowercase() == name.to_lowercase())
        {
            return invalid("A saved view already has that name.");
        }
        let record = StoredView {
            schema: 1,
            view_id: format!("view-{}", Uuid::new_v4()),
            name,
            definition,
            extra: BTreeMap::new(),
        };
        let path = self
            .root
            .join("views")
            .join(format!("{}.md", record.view_id));
        validate_real_collection_ancestors(&self.root, &path)?;
        fs::create_dir_all(self.root.join("views"))?;
        let mut body = format!("# {}\n\nThis named view evaluates its YAML criteria against current knowledge whenever opened. No old result set is stored. Unsaved exploration and automatic session position are separate.\n\n## Referenced material\n\n", super::escape_markdown(&record.name));
        for (source, id) in record.definition.references()? {
            let target = if source {
                self.open_source(&id)
                    .ok()
                    .and_then(|p| self.page_path(&id).ok().map(|path| (p.info.title, path)))
            } else {
                self.open_knowledge_page(&id)
                    .ok()
                    .and_then(|p| self.knowledge_path(&id).ok().map(|path| (p.title, path)))
            };
            if let Some((title, path)) = target {
                let relative = relative(&self.root, &path);
                const LINK_CHARS: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
                    .remove(b'/')
                    .remove(b'-')
                    .remove(b'_')
                    .remove(b'.')
                    .remove(b'~');
                let encoded =
                    percent_encoding::utf8_percent_encode(&relative, LINK_CHARS).to_string();
                body.push_str(&format!(
                    "- [{}](../{}) — `{id}`\n",
                    super::escape_markdown(&title),
                    encoded
                ));
            } else {
                body.push_str(&format!(
                    "- Unavailable reference `{id}`; its stored scope is unchanged.\n"
                ));
            }
        }
        if !record.definition.trail.is_empty() {
            body.push_str("\n## Ordered trail\n\n");
            for (index, stop) in record.definition.trail.iter().enumerate() {
                body.push_str(&format!(
                    "{}. {} — stable stop `{}`\n",
                    index + 1,
                    super::escape_markdown(&stop.label),
                    stop.stop_id
                ));
                if let Some(context) = &stop.context {
                    let location = match context {
                        super::trails::TrailContext::Span {
                            basis,
                            byte_start,
                            byte_end,
                            ..
                        } => format!("{basis}, bytes {byte_start}–{byte_end}"),
                        super::trails::TrailContext::Message { message_id, .. } => {
                            format!("Message {message_id}")
                        }
                        super::trails::TrailContext::Media { at_ms, .. } => {
                            format!("Media {at_ms} ms")
                        }
                    };
                    body.push_str(&format!("   Context: {}. Open in the app to validate its current availability; source-version identity and exact criteria are retained in YAML.\n",super::escape_markdown(&location)));
                }
            }
        }
        super::commit_publication(
            &self.root,
            vec![(path, serialize_view(&record, &body)?)],
            &HashMap::new(),
            None,
        )?;
        self.saved_view_dto(record)
    }
    pub fn rename_saved_view(&mut self, id: &str, name: &str) -> Result<SavedView> {
        let (path, mut record, body, expected) = self.read_saved_view(id)?;
        let name = view_name(name)?;
        if self
            .view_catalog()?
            .rows
            .iter()
            .any(|(_, r)| r.view_id != id && r.name.to_lowercase() == name.to_lowercase())
        {
            return invalid("A saved view already has that name.");
        }
        let body = body
            .strip_prefix(&format!("# {}\n", super::escape_markdown(&record.name)))
            .map(|rest| format!("# {}\n{rest}", super::escape_markdown(&name)))
            .unwrap_or(body);
        record.name = name;
        super::commit_publication(
            &self.root,
            vec![(path.clone(), serialize_view(&record, &body)?)],
            &HashMap::from([(path, expected)]),
            None,
        )?;
        self.saved_view_dto(record)
    }
    pub fn remove_saved_view(&mut self, id: &str) -> Result<()> {
        let (path, _, _, expected) = self.read_saved_view(id)?;
        let trash = self
            .root
            .join(".trash/views")
            .join(format!("{id}-{}.md", Uuid::new_v4()));
        validate_real_collection_ancestors(&self.root, &trash)?;
        fs::create_dir_all(self.root.join(".trash/views"))?;
        commit_publication_with_removals(
            &self.root,
            vec![(trash, expected.clone())],
            vec![path.clone()],
            &HashMap::from([(path, expected)]),
            None,
        )
    }
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
