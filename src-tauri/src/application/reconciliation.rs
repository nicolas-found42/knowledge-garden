//! Reconcile ordinary Markdown by durable identity; no watcher event is authoritative.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollectionDiagnostic {
    pub kind: String,
    pub paths: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReconciliationStatus {
    pub complete: bool,
    pub revision: u64,
    pub diagnostics: Vec<CollectionDiagnostic>,
}

#[derive(Default)]
pub(super) struct Scan {
    directories: Vec<fs::ReadDir>,
    files: BTreeMap<String, String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    pages: BTreeMap<String, String>,
}

impl Application {
    /// Visit at most `budget` directory entries. Complete generations repair links
    /// and refresh derived navigation; incomplete scans never infer deletions.
    pub fn reconcile_collection(&mut self, budget: usize) -> Result<ReconciliationStatus> {
        recover_publications(&self.root)?;
        if self.collection_scan.is_none() {
            let mut scan = Scan::default();
            for directory in ["pages", "sources"] {
                match fs::read_dir(self.root.join(directory)) {
                    Ok(entries) => scan.directories.push(entries),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
            self.collection_scan = Some(scan);
        }
        let scan = self.collection_scan.as_mut().unwrap();
        for _ in 0..budget.clamp(1, 1000) {
            let Some(directory) = scan.directories.last_mut() else {
                break;
            };
            let Some(entry) = directory.next() else {
                scan.directories.pop();
                continue;
            };
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    self.collection_scan = None;
                    return Err(error.into());
                }
            };
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if let Ok(entries) = fs::read_dir(entry.path()) {
                    scan.directories.push(entries);
                }
            } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
                let path = entry.path();
                if let Ok(text) = read_bounded_text(&path, MAX_PAGE_BYTES) {
                    scan.files.insert(
                        path.strip_prefix(&self.root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                        text,
                    );
                }
            }
        }
        if !scan.directories.is_empty() {
            return Ok(self.reconciliation_status(false));
        }
        let scan = self.collection_scan.take().unwrap();
        let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&scan.files)?));
        if self.collection_digest.as_ref() != Some(&digest) {
            self.reconcile_snapshot(scan.files)?;
            self.rebuild_index()?;
            self.collection_digest = Some(digest);
            self.collection_revision += 1;
        }
        Ok(self.reconciliation_status(true))
    }

    pub fn collection_status(&self) -> ReconciliationStatus {
        self.reconciliation_status(self.collection_scan.is_none())
    }

    fn reconciliation_status(&self, complete: bool) -> ReconciliationStatus {
        ReconciliationStatus {
            complete,
            revision: self.collection_revision,
            diagnostics: self.collection_diagnostics.clone(),
        }
    }

    fn reconcile_snapshot(&mut self, files: BTreeMap<String, String>) -> Result<()> {
        let ledger_path = self.root.join(".reconciliation/paths.md");
        let previous: Ledger = if ledger_path.exists() {
            let text = read_bounded_text(&ledger_path, MAX_PAGE_BYTES)?;
            serde_yaml_ng::from_str(
                text.strip_prefix("---\n")
                    .and_then(|v| v.split_once("\n---\n"))
                    .ok_or_else(|| {
                        GardenError::Invalid(
                            "The page location ledger is unreadable; original files are preserved."
                                .into(),
                        )
                    })?
                    .0,
            )?
        } else {
            Ledger::default()
        };
        let mut candidates = BTreeMap::<String, Vec<String>>::new();
        let mut diagnostics = Vec::new();
        for (path, text) in &files {
            if path.starts_with("sources/") && path.ends_with("/index.md") {
                if read_page(&self.root.join(path)).is_err() {
                    diagnostics.push(CollectionDiagnostic { kind: "malformed_metadata".into(), paths: vec![path.clone()], detail: "Source metadata is unreadable; the original file and retained assets remain preserved.".into() });
                }
            }
            if !path.starts_with("pages/") {
                continue;
            }
            match parse_knowledge_header(text) {
                Ok(header) => {
                    if !header.uninterpreted_owner_edits.is_empty() {
                        diagnostics.push(CollectionDiagnostic { kind: "retained_owner_save".into(), paths: header.uninterpreted_owner_edits.clone(), detail: "Exact owner saves retained with uncertainty. Supported manual corrections remain authoritative; unedited facts continue updating.".into() });
                    }
                    candidates
                        .entry(header.page_id)
                        .or_default()
                        .push(path.clone());
                }
                Err(_) => diagnostics.push(CollectionDiagnostic {
                    kind: "malformed_metadata".into(),
                    paths: vec![path.clone()],
                    detail:
                        "No unambiguous page identity can be read. Original content is preserved."
                            .into(),
                }),
            }
        }
        let mut ledger = Ledger {
            pages: previous.pages.clone(),
        };
        let mut aliases = BTreeMap::<String, String>::new();
        for (id, paths) in &candidates {
            if paths.len() != 1 {
                diagnostics.push(CollectionDiagnostic { kind: "duplicate_identity".into(), paths: paths.clone(), detail: format!("{id} occurs in multiple files. Neither file is selected for publication or navigation.") });
                continue;
            }
            let path = &paths[0];
            aliases.insert(format!("pages/{id}.md"), path.clone());
            if let Some(old) = previous.pages.get(id) {
                aliases.insert(old.clone(), path.clone());
            }
            ledger.pages.insert(id.clone(), path.clone());
        }
        // If an old location has been reused, the link is ambiguous, not evidence
        // that the new occupant should become the target of an old incoming link.
        aliases.retain(|old, new| old == new || !files.contains_key(old));
        let mut writes = Vec::new();
        // A replacement during a scan is not a complete generation. Retry from
        // the current tree instead of inferring deletion from stale entries.
        for (path, text) in &files {
            if fs::read(self.root.join(path)).ok().as_deref() != Some(text.as_bytes()) {
                self.collection_digest = None;
                return Ok(());
            }
        }
        for (path, text) in &files {
            let is_ambiguous = candidates
                .values()
                .any(|paths| paths.len() > 1 && paths.contains(path));
            if is_ambiguous
                || (path.starts_with("pages/")
                    && text.starts_with("---\n")
                    && parse_knowledge_header(text).is_err())
            {
                continue;
            }
            let origin = previous
                .pages
                .iter()
                .find_map(|(id, old)| {
                    candidates
                        .get(id)
                        .filter(|paths| paths.len() == 1 && paths[0] == *path && old != path)
                        .map(|_| old.as_str())
                })
                .unwrap_or(path);
            let rebased = rebase_links(origin, path, text);
            let mut repaired = repair_links(path, &rebased, &aliases, &files, &mut diagnostics);
            if path.starts_with("sources/") && path.ends_with("/index.md") {
                if let Ok(mut page) = read_page(&self.root.join(path)) {
                    let mut changed = false;
                    for summary in &mut page.info.knowledge_pages {
                        if let Some(paths) = candidates
                            .get(&summary.page_id)
                            .filter(|paths| paths.len() == 1)
                        {
                            let header = parse_knowledge_header(&files[&paths[0]])?;
                            changed |= summary.path != paths[0] || summary.title != header.title;
                            summary.path = paths[0].clone();
                            summary.title = header.title;
                        }
                    }
                    if changed {
                        let body = repaired
                            .split_once("\n---\n")
                            .map(|(_, body)| body)
                            .unwrap_or(&page.body);
                        repaired = serialize_page(&page.info, body)?;
                    }
                }
            }
            if repaired != *text {
                writes.push((self.root.join(path), repaired.into_bytes()));
            }
        }
        let ledger_text = format!("---\n{}---\n\n# Page locations\n\nLast unambiguous paths, used to repair ordinary links after moves. Markdown pages remain authoritative.\n", serde_yaml_ng::to_string(&ledger)?);
        writes.push((ledger_path, ledger_text.into_bytes()));
        commit_publication(
            &self.root,
            writes,
            &files
                .iter()
                .map(|(path, text)| (self.root.join(path), text.as_bytes().to_vec()))
                .collect(),
            self.fail_after_publication_files.take(),
        )?;
        self.collection_diagnostics = diagnostics;
        Ok(())
    }

    pub fn collection_file_path(&self, relative: &str) -> Result<PathBuf> {
        let path = Path::new(relative);
        validate_relative_collection_path(path)?;
        if !["pages", "sources", "owner-edits"]
            .iter()
            .any(|prefix| path.starts_with(prefix))
            || path.extension().is_none_or(|ext| ext != "md")
        {
            return Err(GardenError::Invalid(
                "Only preserved collection Markdown can be opened here.".into(),
            ));
        }
        let canonical = fs::canonicalize(self.root.join(path))?;
        if !canonical.starts_with(&self.root) || !canonical.is_file() {
            return Err(GardenError::Invalid(
                "This file is outside the collection.".into(),
            ));
        }
        Ok(canonical)
    }

    /// Resolve a relative ordinary Markdown link at the reader boundary.
    pub fn resolve_page_link(&self, page_id: &str, href: &str) -> Result<String> {
        let from = self.knowledge_path(page_id)?;
        let relative = from.strip_prefix(&self.root).unwrap().to_string_lossy();
        let target = resolve_relative(&relative, href)
            .ok_or_else(|| GardenError::Invalid("This link is outside the collection.".into()))?;
        let mut found = None;
        for path in self.knowledge_paths()? {
            if path == self.root.join(&target) {
                found = Some(read_knowledge_header(&path)?.page_id);
            }
        }
        found.ok_or_else(|| {
            GardenError::Invalid(
                "This link has a missing or ambiguous target. Original Markdown is preserved."
                    .into(),
            )
        })
    }
}

fn resolve_relative(from: &str, href: &str) -> Option<String> {
    let href = href.split(['#', '?']).next()?;
    let href = percent_encoding::percent_decode_str(href)
        .decode_utf8()
        .ok()?;
    if href.is_empty() || href.starts_with('/') || href.contains(':') || href.contains('\\') {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in href.split('/') {
        match part {
            ".." => {
                parts.pop()?;
            }
            "." | "" => {}
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

fn relative_link(from: &str, target: &str) -> String {
    let mut from: Vec<_> = from.split('/').collect();
    from.pop();
    let to: Vec<_> = target.split('/').collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let path = std::iter::repeat_n("..", from.len() - common)
        .chain(to[common..].iter().copied())
        .collect::<Vec<_>>()
        .join("/");
    path.replace('%', "%25")
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
        .replace('#', "%23")
        .replace('?', "%3F")
}

fn repair_links(
    from: &str,
    text: &str,
    aliases: &BTreeMap<String, String>,
    files: &BTreeMap<String, String>,
    diagnostics: &mut Vec<CollectionDiagnostic>,
) -> String {
    let links = regex::Regex::new(r"\]\(([^\s)]+)([^)]*)\)|(?m)^\[[^\]\n]+\]:\s*(\S+)").unwrap();
    links.replace_all(text, |capture: &regex::Captures<'_>| {
        let matched = capture.get(1).or_else(|| capture.get(3)).unwrap();
        let href = matched.as_str();
        let Some(target) = resolve_relative(from, href) else { return capture[0].to_owned() };
        if let Some(destination) = aliases.get(&target) {
            let suffix = href.find(['#','?']).map(|at| &href[at..]).unwrap_or("");
            let replaced = format!("{}{suffix}", relative_link(from, destination));
            return capture[0].replacen(href, &replaced, 1);
        }
        if target.ends_with(".md") && !files.contains_key(&target) {
            diagnostics.push(CollectionDiagnostic { kind: "unresolved_target".into(), paths: vec![from.into(), target], detail: "The ordinary Markdown target is missing or ambiguous; the link was preserved.".into() });
        }
        capture[0].to_owned()
    }).into_owned()
}

pub(super) fn rebase_links(from: &str, to: &str, text: &str) -> String {
    if from == to {
        return text.to_owned();
    }
    let links = regex::Regex::new(r"\]\(([^\s)]+)([^)]*)\)|(?m)^\[[^\]\n]+\]:\s*(\S+)").unwrap();
    links
        .replace_all(text, |capture: &regex::Captures<'_>| {
            let matched = capture.get(1).or_else(|| capture.get(3)).unwrap();
            let href = matched.as_str();
            let Some(target) = resolve_relative(from, href) else {
                return capture[0].to_owned();
            };
            let suffix = href.find(['#', '?']).map(|at| &href[at..]).unwrap_or("");
            capture[0].replacen(href, &format!("{}{suffix}", relative_link(to, &target)), 1)
        })
        .into_owned()
}
