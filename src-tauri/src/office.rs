//! Bounded, read-only projection of common modern OOXML Word and PowerPoint files.
//! The archive remains the authoritative original; this module does not follow links.
use std::{collections::HashMap, io::Read, path::Path};
use zip::ZipArchive;

const MAX_PART_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_RELATIONSHIP_SCAN_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageScope {
    MainDocument,
    Tables,
    SlideText,
    SpeakerNotes,
    EmbeddedObject,
    AudioRecording,
    ImagePixels,
    ImageMetadata,
    ImageText,
    ImageInterpretation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Complete,
    Partial,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoveragePart {
    pub scope: CoverageScope,
    pub status: CoverageStatus,
    pub source_location: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct OfficeProjection {
    pub markdown: String,
    /// A located text projection passed to semantic providers. Each evidence line is
    /// prefixed with an explicit original-part locator and channel.
    pub semantic_text: String,
    pub line_count: usize,
    pub coverage: Vec<CoveragePart>,
    pub partial: bool,
    pub detail: String,
}

#[derive(Debug, Clone)]
struct Relationship {
    target: String,
    kind: String,
    external: bool,
}

pub fn extract(path: &Path, format: &str, title: &str) -> Result<OfficeProjection, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file)
        .map_err(|_| "The Office file is not a readable ZIP package.".to_owned())?;
    if archive.len() > MAX_ENTRIES {
        return Err("The Office package contains too many parts to inspect safely.".into());
    }
    match format {
        "docx" => extract_docx(&mut archive, title),
        "pptx" => extract_pptx(&mut archive, title),
        _ => Err("Only modern DOCX and PPTX packages are supported.".into()),
    }
}

fn read_part(archive: &mut ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>, String> {
    let part = archive
        .by_name(name)
        .map_err(|_| format!("Required Office part `{name}` is missing."))?;
    if part.size() > MAX_PART_BYTES {
        return Err(format!(
            "Office part `{name}` exceeds the 8 MiB part limit."
        ));
    }
    let mut bytes = Vec::with_capacity(part.size() as usize);
    part.take(MAX_PART_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Office part `{name}` could not be read: {e}"))?;
    if bytes.len() as u64 > MAX_PART_BYTES {
        return Err(format!(
            "Office part `{name}` exceeds the 8 MiB part limit."
        ));
    }
    Ok(bytes)
}

fn parse_xml<'input>(
    bytes: &'input [u8],
    part: &str,
) -> Result<roxmltree::Document<'input>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| format!("Office part `{part}` is not UTF-8 XML."))?;
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err(format!(
            "Office part `{part}` contains a DTD or entity declaration that is not accepted."
        ));
    }
    roxmltree::Document::parse(text)
        .map_err(|e| format!("Office part `{part}` is malformed XML: {e}"))
}

fn local_name<'a, 'input>(node: roxmltree::Node<'a, 'input>) -> &'input str {
    node.tag_name().name()
}

fn child_named<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Option<roxmltree::Node<'a, 'input>> {
    node.children()
        .find(|child| child.is_element() && local_name(*child) == name)
}

fn children_named<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    name: &str,
) -> Vec<roxmltree::Node<'a, 'input>> {
    node.children()
        .filter(move |child| child.is_element() && local_name(*child) == name)
        .collect()
}

fn attr_local(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.attributes()
        .find(|attr| attr.name() == name)
        .map(|attr| attr.value().to_owned())
}

fn rels_name(part: &str) -> String {
    let (folder, file) = part.rsplit_once('/').unwrap_or(("", part));
    if folder.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{folder}/_rels/{file}.rels")
    }
}

fn relationships(
    archive: &mut ZipArchive<std::fs::File>,
    part: &str,
) -> Result<HashMap<String, Relationship>, String> {
    let name = rels_name(part);
    let bytes = match read_part(archive, &name) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(HashMap::new()),
    };
    let doc = parse_xml(&bytes, &name)?;
    Ok(doc
        .root_element()
        .children()
        .filter(|node| node.is_element() && local_name(*node) == "Relationship")
        .filter_map(|node| {
            Some((
                attr_local(node, "Id")?,
                Relationship {
                    target: attr_local(node, "Target")?,
                    kind: attr_local(node, "Type").unwrap_or_default(),
                    external: attr_local(node, "TargetMode").as_deref() == Some("External"),
                },
            ))
        })
        .collect())
}

fn resolve_target(base_part: &str, target: &str) -> Option<String> {
    if target.contains(':') || target.starts_with("//") {
        return None;
    }
    let mut pieces: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        base_part
            .rsplit_once('/')
            .map(|(folder, _)| folder.split('/').collect())
            .unwrap_or_default()
    };
    for piece in target.trim_start_matches('/').split('/') {
        match piece {
            "" | "." => {}
            ".." => {
                pieces.pop()?;
            }
            value => pieces.push(value),
        }
    }
    Some(pieces.join("/"))
}

fn node_text(node: roxmltree::Node<'_, '_>) -> String {
    let mut out = String::new();
    for child in node.descendants().filter(|n| n.is_element() || n.is_text()) {
        if child.is_text() {
            out.push_str(child.text().unwrap_or(""));
        } else if child.is_element() && local_name(child) == "tab" {
            out.push('\t');
        } else if child.is_element() && matches!(local_name(child), "br" | "cr") {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn safe_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn external_links(rels: &HashMap<String, Relationship>) -> Vec<String> {
    let mut links: Vec<_> = rels
        .values()
        .filter(|rel| rel.external && rel.kind.ends_with("/hyperlink"))
        .map(|rel| rel.target.clone())
        .collect();
    links.sort();
    links.dedup();
    links
}

fn external_hyperlink_target<'a>(
    rels: &'a HashMap<String, Relationship>,
    relationship_id: &str,
) -> Option<&'a str> {
    rels.get(relationship_id)
        .filter(|rel| rel.external && rel.kind.ends_with("/hyperlink"))
        .map(|rel| rel.target.as_str())
}

fn word_hyperlinks(
    paragraph: roxmltree::Node<'_, '_>,
    rels: &HashMap<String, Relationship>,
) -> Vec<(String, String)> {
    paragraph
        .descendants()
        .filter(|node| node.is_element() && local_name(*node) == "hyperlink")
        .filter_map(|node| {
            let relationship_id = attr_local(node, "id")?;
            let target = external_hyperlink_target(rels, &relationship_id)?;
            let text = node_text(node);
            (!text.is_empty()).then(|| (text, target.to_owned()))
        })
        .collect()
}

fn powerpoint_hyperlinks(
    shape: roxmltree::Node<'_, '_>,
    rels: &HashMap<String, Relationship>,
) -> Vec<(String, String)> {
    shape
        .descendants()
        .filter(|node| node.is_element() && local_name(*node) == "hlinkClick")
        .filter_map(|node| {
            let relationship_id = attr_local(node, "id")?;
            let target = external_hyperlink_target(rels, &relationship_id)?;
            let text = node
                .ancestors()
                .find(|ancestor| local_name(*ancestor) == "r")
                .map(node_text)
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| node_text(shape));
            (!text.is_empty()).then(|| (text, target.to_owned()))
        })
        .collect()
}

fn embedded_coverage(archive: &mut ZipArchive<std::fs::File>) -> Option<CoveragePart> {
    let mut gaps = Vec::new();
    let mut scanned_bytes = 0u64;
    for index in 0..archive.len() {
        let mut part = match archive.by_index(index) {
            Ok(part) => part,
            Err(_) => continue,
        };
        let name = part.name().to_owned();
        if !name.ends_with(".rels") {
            continue;
        }
        scanned_bytes = scanned_bytes.saturating_add(part.size());
        if scanned_bytes > MAX_RELATIONSHIP_SCAN_BYTES {
            gaps.push("relationship metadata scan reached its 8 MiB limit; remaining embedded references were not inventoried".into());
            break;
        }
        let mut bytes = Vec::new();
        if part.size() > MAX_PART_BYTES || part.read_to_end(&mut bytes).is_err() {
            gaps.push(format!("relationship metadata `{name}` is unavailable; embedded references in this part could not be inventoried"));
            continue;
        }
        drop(part);
        let doc = match parse_xml(&bytes, &name) {
            Ok(doc) => doc,
            Err(_) => {
                gaps.push(format!("relationship metadata `{name}` is malformed; embedded references in this part could not be inventoried"));
                continue;
            }
        };
        let Some((base, _)) = name.split_once("/_rels/") else {
            continue;
        };
        let owner = format!(
            "{base}/{}",
            name.rsplit('/')
                .next()
                .unwrap_or("")
                .trim_end_matches(".rels")
        );
        for rel in doc
            .root_element()
            .children()
            .filter(|n| n.is_element() && local_name(*n) == "Relationship")
        {
            let kind = attr_local(rel, "Type").unwrap_or_default();
            let is_ole = kind.ends_with("/oleObject") || kind.ends_with("/package");
            let is_unhandled_media = kind.ends_with("/image")
                || kind.ends_with("/chart")
                || kind.ends_with("/diagramData")
                || kind.ends_with("/media");
            if !is_ole && !is_unhandled_media {
                continue;
            }
            let target = attr_local(rel, "Target").unwrap_or_default();
            match resolve_target(&owner, &target) {
                Some(target_path) => {
                    let readable = archive
                        .by_name(&target_path)
                        .map(|mut item| {
                            if !is_ole {
                                return true;
                            }
                            let mut header = [0u8; 8];
                            item.read_exact(&mut header).is_ok()
                                && header == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]
                        })
                        .unwrap_or(false);
                    if !readable {
                        gaps.push(format!(
                            "{target_path} is missing or is not a readable {}",
                            if is_ole {
                                "OLE compound file"
                            } else {
                                "embedded media part"
                            }
                        ));
                    } else if !is_ole {
                        gaps.push(format!("{target_path} is retained but its chart, diagram, or image content is not interpreted"));
                    } else {
                        gaps.push(format!("{target_path} is retained but its embedded object contents are not extracted"));
                    }
                }
                None => gaps.push(format!("embedded object target `{target}` is invalid")),
            }
        }
    }
    (!gaps.is_empty()).then(|| CoveragePart {
        scope: CoverageScope::EmbeddedObject,
        status: CoverageStatus::Partial,
        source_location: Some("package relationships".into()),
        detail: gaps.join("; "),
    })
}

fn unsupported_document_parts(rels: &HashMap<String, Relationship>) -> Option<CoveragePart> {
    let mut gaps: Vec<_> = rels
        .values()
        .filter(|rel| {
            [
                "/header",
                "/footer",
                "/footnotes",
                "/endnotes",
                "/comments",
                "/customXml",
            ]
            .iter()
            .any(|suffix| rel.kind.ends_with(suffix))
        })
        .map(|rel| {
            format!(
                "{} ({})",
                rel.target,
                rel.kind.rsplit('/').next().unwrap_or("unknown part")
            )
        })
        .collect();
    gaps.sort();
    gaps.dedup();
    (!gaps.is_empty()).then(|| CoveragePart {
        scope: CoverageScope::MainDocument,
        status: CoverageStatus::Partial,
        source_location: Some("word/document.xml relationships".into()),
        detail: format!(
            "Additional text-bearing or custom Office parts were not projected: {}.",
            gaps.join(", ")
        ),
    })
}

fn finish(
    title: &str,
    sections: Vec<String>,
    semantic: Vec<String>,
    mut coverage: Vec<CoveragePart>,
    detail: String,
) -> OfficeProjection {
    let title = title.replace(['\n', '\r'], " ");
    let mut markdown = format!(
        "# {title}\n\n[Open original](ORIGINAL_ASSET)\n\n{}",
        sections.join("\n\n")
    );
    let semantic_text = semantic.join("\n");
    if markdown.len() > 5 * 1024 * 1024 || semantic_text.len() > 2 * 1024 * 1024 {
        return OfficeProjection { markdown: format!("# {title}\n\n[Open original](ORIGINAL_ASSET)\n\nOffice package text exceeds the 2 MiB extracted-text limit.\n\n## Extraction coverage gaps\n\n- Main document text projection exceeded its safe size limit."), semantic_text: String::new(), line_count: 0, coverage: vec![CoveragePart { scope: CoverageScope::MainDocument, status: CoverageStatus::Partial, source_location: Some("package".into()), detail: "The supported text projection exceeded its size limit.".into() }], partial: true, detail: "The original Office package is retained; extracted text exceeds the safe projection limit.".into() };
    }
    let line_count = semantic_text.lines().count();
    if coverage.is_empty() {
        coverage.push(CoveragePart {
            scope: CoverageScope::MainDocument,
            status: CoverageStatus::Complete,
            source_location: Some("document".into()),
            detail: "All supported document text was projected.".into(),
        });
    }
    let gap_parts: Vec<_> = coverage
        .iter()
        .filter(|part| part.status != CoverageStatus::Complete)
        .collect();
    let partial = !gap_parts.is_empty();
    if !gap_parts.is_empty() {
        markdown.push_str("\n\n## Extraction coverage gaps\n\n");
        for gap in &gap_parts {
            markdown.push_str(&format!(
                "- **{} ({})** at {}: {}\n",
                coverage_scope_label(gap.scope),
                format!("{:?}", gap.status).to_lowercase(),
                gap.source_location.as_deref().unwrap_or("unknown location"),
                gap.detail
            ));
        }
    }
    OfficeProjection {
        markdown,
        semantic_text,
        line_count,
        coverage,
        partial,
        detail,
    }
}

fn coverage_scope_label(scope: CoverageScope) -> &'static str {
    match scope {
        CoverageScope::MainDocument => "main document",
        CoverageScope::Tables => "tables",
        CoverageScope::SlideText => "slide text",
        CoverageScope::SpeakerNotes => "speaker notes",
        CoverageScope::EmbeddedObject => "embedded object",
        CoverageScope::AudioRecording => "audio recording",
        CoverageScope::ImagePixels => "image pixels",
        CoverageScope::ImageMetadata => "image metadata",
        CoverageScope::ImageText => "image OCR text",
        CoverageScope::ImageInterpretation => "uncertain image interpretation",
    }
}

fn extract_docx(
    archive: &mut ZipArchive<std::fs::File>,
    title: &str,
) -> Result<OfficeProjection, String> {
    let part = "word/document.xml";
    let bytes = read_part(archive, part)?;
    let doc = parse_xml(&bytes, part)?;
    let (rels, rels_error) = match relationships(archive, part) {
        Ok(rels) => (rels, None),
        Err(error) => (HashMap::new(), Some(error)),
    };
    let body = doc
        .descendants()
        .find(|n| n.is_element() && local_name(*n) == "body")
        .ok_or("DOCX body part is missing.")?;
    let mut sections = vec!["## Document".to_owned()];
    let mut semantic = Vec::new();
    let mut located_links = Vec::new();
    let mut paragraph_no = 0usize;
    let mut table_no = 0usize;
    for node in body.children().filter(|n| n.is_element()) {
        match local_name(node) {
            "p" => {
                let text = node_text(node);
                if text.is_empty() {
                    continue;
                }
                paragraph_no += 1;
                let style = child_named(node, "pPr")
                    .and_then(|ppr| child_named(ppr, "pStyle"))
                    .and_then(|s| attr_local(s, "val"))
                    .unwrap_or_default();
                let para_id = attr_local(node, "paraId");
                let heading_level = style
                    .strip_prefix("Heading")
                    .and_then(|s| s.parse::<usize>().ok())
                    .filter(|n| (1..=6).contains(n));
                let location = format!(
                    "DOCX paragraph {paragraph_no}{}{}",
                    if style.is_empty() {
                        String::new()
                    } else {
                        format!(" style {style}")
                    },
                    para_id
                        .map(|id| format!(" OOXML part {part} paragraph id {id}"))
                        .unwrap_or_default()
                );
                if let Some(level) = heading_level {
                    sections.push(format!("{} {}", "#".repeat((level + 1).min(6)), text));
                } else {
                    sections.push(format!("{text}  \n<!-- {location} -->"));
                }
                semantic.push(format!("[{location}; channel=main_document] {text}"));
                for (index, (anchor, target)) in
                    word_hyperlinks(node, &rels).into_iter().enumerate()
                {
                    let link_location = format!(
                        "{location} hyperlink occurrence {}; channel=reference",
                        index + 1
                    );
                    semantic.push(format!(
                        "[{link_location}] {text} Link text: {anchor}. Destination: {target}"
                    ));
                    located_links.push((target, anchor, text.clone(), link_location));
                }
            }
            "tbl" => {
                table_no += 1;
                let rows: Vec<Vec<String>> = children_named(node, "tr")
                    .into_iter()
                    .map(|row| {
                        children_named(row, "tc")
                            .into_iter()
                            .map(|cell| {
                                cell.descendants()
                                    .filter(|n| n.is_element() && local_name(*n) == "p")
                                    .map(node_text)
                                    .filter(|s| !s.is_empty())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .collect()
                    })
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                sections.push(format!(
                    "### Table {table_no}\n\n{}",
                    rows.iter()
                        .enumerate()
                        .map(|(idx, row)| {
                            let cells = row
                                .iter()
                                .map(|cell| safe_cell(cell))
                                .collect::<Vec<_>>()
                                .join(" | ");
                            if idx == 0 {
                                format!(
                                    "| {cells} |\n| {} |",
                                    row.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
                                )
                            } else {
                                format!("| {cells} |")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
                for (row_idx, row) in rows.iter().enumerate() {
                    // The first row is the header row used by the Markdown table above; do not
                    // offer it to semantic processing as a factual data row.
                    if row_idx == 0 {
                        continue;
                    }
                    let columns = rows
                        .first()
                        .map(|header| {
                            header
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, head)| {
                                    row.get(idx).map(|value| format!("{}: {}", head, value))
                                })
                                .collect::<Vec<_>>()
                                .join("; ")
                        })
                        .unwrap_or_default();
                    let row_no = row_idx + 1;
                    semantic.push(format!(
                        "[DOCX table {table_no} row {row_no}; channel=table] {columns}"
                    ));
                }
            }
            _ => {}
        }
    }
    let links = external_links(&rels);
    if !links.is_empty() {
        let occurrences = located_links
            .iter()
            .map(|(url, anchor, context, location)| {
                format!("- `{url}` — {anchor}; occurrence in {location}: {context}")
            })
            .collect::<Vec<_>>();
        sections.push(format!(
            "## Unfetched links\n\n{}{}",
            links
                .iter()
                .map(|url| format!(
                    "- `{}` (reference only; destination not visited)",
                    url.replace('`', "")
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            if occurrences.is_empty() {
                String::new()
            } else {
                format!("\n\n### Located occurrences\n\n{}", occurrences.join("\n"))
            }
        ));
    }
    let unsupported_parts = unsupported_document_parts(&rels);
    let mut coverage = vec![CoveragePart {
        scope: CoverageScope::MainDocument,
        status: if unsupported_parts.is_some() || rels_error.is_some() {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Complete
        },
        source_location: Some(part.into()),
        detail: if let Some(error) = rels_error.as_deref() {
            format!("Projected {paragraph_no} paragraphs; relationship links and related parts may be missing: {error}")
        } else {
            format!(
                "Projected {paragraph_no} paragraphs in source order; headings remain headings."
            )
        },
    }];
    if let Some(gap) = unsupported_parts {
        coverage.push(gap);
    }
    coverage.push(CoveragePart { scope: CoverageScope::Tables, status: CoverageStatus::Complete, source_location: Some(part.into()), detail: format!("Projected {table_no} tables in row and cell order; row labels are retained with cell values." ) });
    if let Some(gap) = embedded_coverage(archive) {
        coverage.push(gap);
    }
    let gaps = coverage
        .iter()
        .any(|part| part.status != CoverageStatus::Complete);
    let detail = if gaps { "Office text is readable; one or more embedded objects are retained as explicit coverage gaps." } else { "Office document structure and source order are preserved." }.to_owned();
    Ok(finish(title, sections, semantic, coverage, detail))
}

fn extract_pptx(
    archive: &mut ZipArchive<std::fs::File>,
    title: &str,
) -> Result<OfficeProjection, String> {
    let presentation_part = "ppt/presentation.xml";
    let bytes = read_part(archive, presentation_part)?;
    let presentation = parse_xml(&bytes, presentation_part)?;
    let (presentation_rels, presentation_rels_error) =
        match relationships(archive, presentation_part) {
            Ok(rels) => (rels, None),
            Err(error) => (HashMap::new(), Some(error)),
        };
    let slide_list = presentation
        .descendants()
        .find(|n| n.is_element() && local_name(*n) == "sldIdLst")
        .ok_or("PPTX slide order list is missing.")?;
    let mut sections = Vec::new();
    let mut semantic = Vec::new();
    let mut notes_found = 0usize;
    let mut expected_notes = 0usize;
    let mut slide_text_partial = presentation_rels_error.is_some();
    let mut notes_partial = presentation_rels_error.is_some();
    if let Some(error) = presentation_rels_error.as_deref() {
        sections.push(format!(
            "## Presentation relationship coverage gap\n\n{error}"
        ));
    }
    for (slide_index, slide_id) in children_named(slide_list, "sldId").into_iter().enumerate() {
        let slide_no = slide_index + 1;
        let relationship_id = slide_id
            .attributes()
            .map(|attr| attr.value())
            .find(|value| presentation_rels.contains_key(*value))
            .unwrap_or_default();
        let Some(rel) = presentation_rels.get(relationship_id) else {
            slide_text_partial = true;
            notes_partial = true;
            sections.push(format!("## Slide {slide_no}\n\nSlide relationship `{relationship_id}` is unavailable; slide text could not be located."));
            continue;
        };
        let Some(slide_part) = resolve_target(presentation_part, &rel.target) else {
            slide_text_partial = true;
            notes_partial = true;
            sections.push(format!(
                "## Slide {slide_no}\n\nSlide relationship target is invalid."
            ));
            continue;
        };
        let slide_bytes = match read_part(archive, &slide_part) {
            Ok(bytes) => bytes,
            Err(e) => {
                slide_text_partial = true;
                notes_partial = true;
                sections.push(format!(
                    "## Slide {slide_no}\n\nSlide text unavailable: {e}"
                ));
                continue;
            }
        };
        let slide = match parse_xml(&slide_bytes, &slide_part) {
            Ok(doc) => doc,
            Err(e) => {
                slide_text_partial = true;
                notes_partial = true;
                sections.push(format!(
                    "## Slide {slide_no}\n\nSlide text unavailable: {e}"
                ));
                continue;
            }
        };
        let mut visible = Vec::new();
        let mut ordered_content = Vec::new();
        let mut slide_table_count = 0usize;
        let shape_tree = slide
            .descendants()
            .find(|n| n.is_element() && local_name(*n) == "spTree");
        for shape in shape_tree
            .into_iter()
            .flat_map(|tree| tree.children())
            .filter(|n| n.is_element())
        {
            match local_name(shape) {
                "sp" | "cxnSp" => {
                    let text = shape
                        .descendants()
                        .filter(|n| n.is_element() && local_name(*n) == "t")
                        .filter_map(|n| n.text())
                        .collect::<Vec<_>>()
                        .join(" ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !text.is_empty() {
                        let object_id = shape
                            .descendants()
                            .find(|node| node.is_element() && local_name(*node) == "cNvPr")
                            .and_then(|node| attr_local(node, "id"));
                        let locator = format!(
                            "PPTX slide {slide_no}{} visible text block {}",
                            object_id
                                .as_deref()
                                .map(|id| format!(" OOXML part {slide_part} shape id {id}"))
                                .unwrap_or_default(),
                            visible.len() + 1,
                        );
                        semantic.push(format!("[{locator}; channel=slide_text] {text}"));
                        ordered_content.push(text.clone());
                        visible.push(text);
                    }
                }
                "graphicFrame" => {
                    for table in shape
                        .descendants()
                        .filter(|n| n.is_element() && local_name(*n) == "tbl")
                    {
                        slide_table_count += 1;
                        let object_id = shape
                            .descendants()
                            .find(|node| node.is_element() && local_name(*node) == "cNvPr")
                            .and_then(|node| attr_local(node, "id"));
                        let rows: Vec<Vec<String>> = children_named(table, "tr")
                            .into_iter()
                            .map(|row| {
                                children_named(row, "tc")
                                    .into_iter()
                                    .map(|cell| {
                                        cell.descendants()
                                            .filter(|n| n.is_element() && local_name(*n) == "t")
                                            .filter_map(|n| n.text())
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                            .split_whitespace()
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    })
                                    .collect()
                            })
                            .collect();
                        for (row_index, row) in rows.iter().enumerate() {
                            // The first row is rendered as this table's header, not as a data fact.
                            if row_index == 0 {
                                continue;
                            }
                            let pairs = rows
                                .first()
                                .map(|header| {
                                    header
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(idx, name)| {
                                            row.get(idx).map(|value| format!("{name}: {value}"))
                                        })
                                        .collect::<Vec<_>>()
                                        .join("; ")
                                })
                                .unwrap_or_default();
                            semantic.push(format!("[PPTX slide {slide_no}{} table {slide_table_count} row {}; channel=slide_text_table] {pairs}", object_id.as_deref().map(|id| format!(" OOXML part {slide_part} shape id {id}")).unwrap_or_default(), row_index + 1));
                        }
                        if !rows.is_empty() {
                            ordered_content.push(format!(
                                "### Table {slide_table_count}\n\n{}",
                                rows.iter()
                                    .enumerate()
                                    .map(|(idx, row)| {
                                        let cells = row
                                            .iter()
                                            .map(|cell| safe_cell(cell))
                                            .collect::<Vec<_>>()
                                            .join(" | ");
                                        if idx == 0 {
                                            format!(
                                                "| {cells} |\n| {} |",
                                                row.iter()
                                                    .map(|_| "---")
                                                    .collect::<Vec<_>>()
                                                    .join(" | ")
                                            )
                                        } else {
                                            format!("| {cells} |")
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        sections.push(format!(
            "## Slide {slide_no}\n\n### Visible slide text\n\n{}",
            if ordered_content.is_empty() {
                "No supported visible text was found.".into()
            } else {
                ordered_content.join("\n\n")
            }
        ));

        let slide_rels = match relationships(archive, &slide_part) {
            Ok(rels) => rels,
            Err(error) => {
                notes_partial = true;
                sections.push(format!(
                    "### Slide {slide_no} package relationships unavailable\n\n{error}"
                ));
                HashMap::new()
            }
        };
        let mut located_slide_links = Vec::new();
        for shape in slide
            .descendants()
            .filter(|node| node.is_element() && local_name(*node) == "sp")
        {
            let context = node_text(shape);
            if context.is_empty() {
                continue;
            }
            let object_id = shape
                .descendants()
                .find(|node| node.is_element() && local_name(*node) == "cNvPr")
                .and_then(|node| attr_local(node, "id"))
                .unwrap_or_else(|| "unknown".into());
            for (index, (anchor, target)) in powerpoint_hyperlinks(shape, &slide_rels)
                .into_iter()
                .enumerate()
            {
                let location = format!(
                    "PPTX slide {slide_no} shape {object_id} hyperlink occurrence {}; channel=reference",
                    index + 1
                );
                semantic.push(format!(
                    "[{location}] {context} Link text: {anchor}. Destination: {target}"
                ));
                located_slide_links.push((target, anchor, context.clone(), location));
            }
        }
        if let Some(note_rel) = slide_rels
            .values()
            .find(|r| r.kind.ends_with("/notesSlide"))
        {
            expected_notes += 1;
            if let Some(note_part) = resolve_target(&slide_part, &note_rel.target) {
                if let Ok(note_bytes) = read_part(archive, &note_part) {
                    if let Ok(note_doc) = parse_xml(&note_bytes, &note_part) {
                        let mut notes = Vec::new();
                        for shape in note_doc
                            .descendants()
                            .filter(|n| n.is_element() && local_name(*n) == "sp")
                        {
                            let is_header = shape.descendants().any(|n| {
                                n.is_element()
                                    && local_name(n) == "ph"
                                    && matches!(
                                        attr_local(n, "type").as_deref(),
                                        Some("sldNum" | "dt" | "hdr" | "ftr")
                                    )
                            });
                            if is_header {
                                continue;
                            }
                            let text = shape
                                .descendants()
                                .filter(|n| n.is_element() && local_name(*n) == "t")
                                .filter_map(|n| n.text())
                                .collect::<Vec<_>>()
                                .join(" ")
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            if !text.is_empty() {
                                let object_id = shape
                                    .descendants()
                                    .find(|node| node.is_element() && local_name(*node) == "cNvPr")
                                    .and_then(|node| attr_local(node, "id"));
                                notes.push((text, object_id));
                            }
                        }
                        sections.push(format!(
                            "\n### Speaker notes (not visible slide content)\n\n{}",
                            if notes.is_empty() {
                                "No speaker note text was found.".into()
                            } else {
                                notes
                                    .iter()
                                    .map(|(text, _)| text.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n\n")
                            }
                        ));
                        for (idx, (text, object_id)) in notes.iter().enumerate() {
                            let note_no = idx + 1;
                            semantic.push(format!("[PPTX slide {slide_no} speaker note {note_no}{}; channel=speaker_notes] {text}", object_id.as_deref().map(|id| format!(" OOXML part {note_part} shape id {id}")).unwrap_or_default()));
                        }
                        notes_found += 1;
                    } else {
                        notes_partial = true;
                    }
                } else {
                    notes_partial = true;
                }
            } else {
                notes_partial = true;
            }
        }
        let links = external_links(&slide_rels);
        if !links.is_empty() {
            let occurrences = located_slide_links
                .iter()
                .map(|(url, anchor, context, location)| {
                    format!("- `{url}` — {anchor}; occurrence in {location}: {context}")
                })
                .collect::<Vec<_>>();
            sections.push(format!(
                "\n#### Unfetched links\n\n{}{}",
                links
                    .iter()
                    .map(|url| format!(
                        "- `{}` (reference only; destination not visited)",
                        url.replace('`', "")
                    ))
                    .collect::<Vec<_>>()
                    .join("\n"),
                if occurrences.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n\n##### Located occurrences\n\n{}",
                        occurrences.join("\n")
                    )
                }
            ));
        }
    }
    let slide_count = children_named(slide_list, "sldId").len();
    let mut coverage = vec![CoveragePart { scope: CoverageScope::SlideText, status: if slide_text_partial { CoverageStatus::Partial } else { CoverageStatus::Complete }, source_location: Some(presentation_part.into()), detail: format!("Processed {slide_count} slides in presentation order; visible slide text is separate from speaker notes.") }];
    coverage.push(CoveragePart { scope: CoverageScope::SpeakerNotes, status: if notes_partial || notes_found < expected_notes { CoverageStatus::Partial } else { CoverageStatus::Complete }, source_location: Some("slide notes relationships".into()), detail: format!("Read {notes_found} of {expected_notes} linked notes parts; notes remain distinct from visible slide text.") });
    coverage.push(CoveragePart {
        scope: CoverageScope::Tables,
        status: CoverageStatus::Complete,
        source_location: Some("slide graphic frames".into()),
        detail: "DrawingML tables in visible slides are projected in row and column order.".into(),
    });
    if let Some(gap) = embedded_coverage(archive) {
        coverage.push(gap);
    }
    let gaps = slide_text_partial
        || notes_partial
        || coverage
            .iter()
            .any(|part| part.status != CoverageStatus::Complete);
    let detail = if gaps { "Presentation text is readable; missing slide, notes, or embedded-object content is identified in coverage." } else { "Presentation order, visible slide text, and speaker notes are preserved separately." }.to_owned();
    Ok(finish(title, sections, semantic, coverage, detail))
}
