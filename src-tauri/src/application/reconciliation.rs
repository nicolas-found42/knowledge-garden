//! Reconcile ordinary Markdown by durable identity; no watcher event is authoritative.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound::{Excluded, Unbounded};

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
    directory_mtimes: BTreeMap<PathBuf, std::time::SystemTime>,
    files: BTreeMap<String, String>,
    source_directories: BTreeSet<String>,
    unreadable: Vec<CollectionDiagnostic>,
    candidate_cursor: Option<String>,
    candidate_digest: Option<String>,
    candidates: BTreeMap<String, Vec<String>>,
}

fn is_source_index(path: &str) -> bool {
    let path = Path::new(path);
    path.file_name() == Some(std::ffi::OsStr::new("index.md"))
        && path.parent().and_then(Path::parent) == Some(Path::new("sources"))
}

fn is_source_acquisition(path: &str) -> bool {
    let path = Path::new(path);
    let Some(parent) = path.parent() else {
        return false;
    };
    path.extension() == Some(std::ffi::OsStr::new("md"))
        && parent.file_name() == Some(std::ffi::OsStr::new("acquisitions"))
        && parent.parent().and_then(Path::parent) == Some(Path::new("sources"))
}

#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    pages: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
enum PageCandidate {
    Parsed {
        page_id: String,
        retained_owner_edits: Vec<String>,
    },
    Malformed,
}

// Bump when page-header parsing or its cached fields change.
const PAGE_CANDIDATE_CACHE_VERSION: &str = "page-candidate-v1";

fn scan_directories_unchanged(directories: &BTreeMap<PathBuf, std::time::SystemTime>) -> bool {
    directories.iter().all(|(path, expected)| {
        fs::metadata(path)
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified == *expected)
    })
}

impl Application {
    /// Visit at most `budget` directory entries. Complete generations repair links
    /// and refresh derived navigation; incomplete scans never infer deletions.
    pub fn reconcile_collection(&mut self, budget: usize) -> Result<ReconciliationStatus> {
        self.collection_reconcile_pending = true;
        recover_publications(&self.root)?;
        if self.collection_scan.is_none() {
            let mut scan = Scan::default();
            scan.directory_mtimes
                .insert(self.root.clone(), fs::metadata(&self.root)?.modified()?);
            for directory in ["pages", "sources"] {
                let path = self.root.join(directory);
                let modified = fs::metadata(&path).and_then(|metadata| metadata.modified());
                match fs::read_dir(&path) {
                    Ok(entries) => {
                        scan.directory_mtimes.insert(path, modified?);
                        scan.directories.push(entries);
                    }
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
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    self.collection_scan = None;
                    return Err(error.into());
                }
            };
            if kind.is_dir() {
                if entry.path().parent() == Some(self.root.join("sources").as_path()) {
                    scan.source_directories.insert(
                        entry
                            .path()
                            .strip_prefix(&self.root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
                let entries = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .and_then(|modified| {
                        fs::read_dir(entry.path()).map(|entries| (modified, entries))
                    });
                match entries {
                    Ok((modified, entries)) => {
                        scan.directory_mtimes.insert(entry.path(), modified);
                        scan.directories.push(entries);
                    }
                    Err(error) => {
                        self.collection_scan = None;
                        return Err(error.into());
                    }
                }
            } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
                let path = entry.path();
                let relative = path
                    .strip_prefix(&self.root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                match read_bounded_text(&path, MAX_PAGE_BYTES) {
                    Ok(text) => { scan.files.insert(relative, hex::encode(Sha256::digest(text.as_bytes()))); }
                    Err(_) => scan.unreadable.push(CollectionDiagnostic { kind: "unreadable_markdown".into(), paths: vec![relative], detail: "Markdown could not be read within the supported size or encoding bounds; its original contents remain preserved.".into() }),
                }
            }
        }
        if !scan.directories.is_empty() {
            return Ok(self.reconciliation_status(false));
        }
        let mut scan = self.collection_scan.take().unwrap();
        if scan.candidate_digest.is_none() {
            if !scan_directories_unchanged(&scan.directory_mtimes) {
                return Ok(self.reconciliation_status(false));
            }
            for directory in &scan.source_directories {
                let relative = format!("{directory}/index.md");
                let path = self.root.join(&relative);
                if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_file())
                {
                    scan.unreadable.push(CollectionDiagnostic { kind: "missing_metadata".into(), paths: vec![relative], detail: "Source metadata is missing or is not an ordinary file; retained assets are preserved for recovery.".into() });
                }
            }
            scan.unreadable
                .sort_by(|left, right| left.paths.cmp(&right.paths));
            let digest = hex::encode(Sha256::digest(serde_json::to_vec(&(
                &scan.files,
                &scan.source_directories,
                &scan.unreadable,
            ))?));
            if self.collection_digest.as_ref() == Some(&digest) {
                self.collection_reconcile_pending = false;
                return Ok(self.reconciliation_status(true));
            }
            self.index.execute_batch(
                "CREATE TABLE IF NOT EXISTS reconciliation_page_candidates (
                    generation TEXT NOT NULL,
                    path TEXT NOT NULL,
                    fingerprint TEXT NOT NULL,
                    outcome TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    PRIMARY KEY (generation, path)
                ) WITHOUT ROWID;",
            )?;
            scan.candidate_digest = Some(digest);
            self.collection_scan = Some(scan);
            return Ok(self.reconciliation_status(false));
        }
        for _ in 0..budget.clamp(1, 1000) {
            let next = match &scan.candidate_cursor {
                Some(cursor) => scan
                    .files
                    .range::<str, _>((Excluded(cursor.as_str()), Unbounded))
                    .next(),
                None => scan.files.iter().next(),
            };
            let Some((path, fingerprint)) = next else {
                let generation = format!(
                    "{PAGE_CANDIDATE_CACHE_VERSION}:{}",
                    scan.candidate_digest.as_deref().unwrap()
                );
                let mut pruned = self.index.execute(
                    "DELETE FROM reconciliation_page_candidates WHERE (generation,path) = (SELECT generation,path FROM reconciliation_page_candidates WHERE generation < ?1 ORDER BY generation,path LIMIT 1)",
                    [&generation],
                )?;
                if pruned == 0 {
                    pruned = self.index.execute(
                        "DELETE FROM reconciliation_page_candidates WHERE (generation,path) = (SELECT generation,path FROM reconciliation_page_candidates WHERE generation > ?1 ORDER BY generation,path LIMIT 1)",
                        [&generation],
                    )?;
                }
                if pruned != 0 {
                    continue;
                }
                if !scan_directories_unchanged(&scan.directory_mtimes) {
                    return Ok(self.reconciliation_status(false));
                }
                let digest = scan.candidate_digest.take().unwrap();
                if !self.reconcile_snapshot(
                    scan.files,
                    scan.unreadable,
                    scan.candidates,
                    &scan.directory_mtimes,
                )? {
                    return Ok(self.reconciliation_status(false));
                }
                self.rebuild_index()?;
                self.collection_digest = Some(digest);
                self.collection_revision += 1;
                self.collection_reconcile_pending = false;
                return Ok(self.reconciliation_status(true));
            };
            let path = path.clone();
            let Ok(text) = read_bounded_text(&self.root.join(&path), MAX_PAGE_BYTES) else {
                return Ok(self.reconciliation_status(false));
            };
            if hex::encode(Sha256::digest(text.as_bytes())) != *fingerprint {
                return Ok(self.reconciliation_status(false));
            }
            if path.starts_with("pages/") {
                let generation = format!(
                    "{PAGE_CANDIDATE_CACHE_VERSION}:{}",
                    scan.candidate_digest.as_deref().unwrap()
                );
                let candidate =
                    self.cached_page_candidate(&generation, &path, fingerprint, &text)?;
                match candidate {
                    PageCandidate::Parsed { page_id, retained_owner_edits } => {
                        if !retained_owner_edits.is_empty() {
                            scan.unreadable.push(CollectionDiagnostic { kind: "retained_owner_save".into(), paths: retained_owner_edits, detail: "Exact owner saves retained with uncertainty. Supported manual corrections remain authoritative; unedited facts continue updating.".into() });
                        }
                        scan.candidates.entry(page_id).or_default().push(path.clone());
                    }
                    PageCandidate::Malformed => scan.unreadable.push(CollectionDiagnostic {
                        kind: "malformed_metadata".into(),
                        paths: vec![path.clone()],
                        detail: "No unambiguous page identity can be read. Original content is preserved.".into(),
                    }),
                }
            } else {
                self.inspect_reconciliation_candidate(&path, &mut scan.unreadable);
            }
            scan.candidate_cursor = Some(path);
        }
        self.collection_scan = Some(scan);
        Ok(self.reconciliation_status(false))
    }

    pub fn collection_status(&self) -> ReconciliationStatus {
        self.reconciliation_status(!self.collection_reconcile_pending)
    }

    fn reconciliation_status(&self, complete: bool) -> ReconciliationStatus {
        ReconciliationStatus {
            complete,
            revision: self.collection_revision,
            diagnostics: self.collection_diagnostics.clone(),
        }
    }

    fn cached_page_candidate(
        &self,
        generation: &str,
        path: &str,
        fingerprint: &str,
        text: &str,
    ) -> Result<PageCandidate> {
        let cached: Option<(Option<String>, Option<String>)> = self.index.query_row(
            "SELECT outcome, checksum FROM reconciliation_page_candidates WHERE generation=?1 AND path=?2 AND fingerprint=?3",
            params![generation, path, fingerprint],
            |row| Ok((row.get(0).ok(), row.get(1).ok())),
        ).optional()?;
        if let Some((Some(outcome), Some(checksum))) = cached {
            if hex::encode(Sha256::digest(outcome.as_bytes())) == checksum {
                if let Ok(candidate) = serde_json::from_str(&outcome) {
                    return Ok(candidate);
                }
            }
        }
        let candidate = match parse_knowledge_header(text) {
            Ok(header) => PageCandidate::Parsed {
                page_id: header.page_id,
                retained_owner_edits: header.uninterpreted_owner_edits,
            },
            Err(_) => PageCandidate::Malformed,
        };
        let outcome = serde_json::to_string(&candidate)?;
        let checksum = hex::encode(Sha256::digest(outcome.as_bytes()));
        self.index.execute(
            "INSERT INTO reconciliation_page_candidates(generation,path,fingerprint,outcome,checksum) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(generation,path) DO UPDATE SET fingerprint=excluded.fingerprint,outcome=excluded.outcome,checksum=excluded.checksum",
            params![generation, path, fingerprint, outcome, checksum],
        )?;
        Ok(candidate)
    }

    fn inspect_reconciliation_candidate(
        &self,
        path: &str,
        diagnostics: &mut Vec<CollectionDiagnostic>,
    ) {
        if is_source_acquisition(path) && read_acquisition_record(&self.root.join(path)).is_err() {
            diagnostics.push(CollectionDiagnostic { kind: "malformed_acquisition".into(), paths: vec![path.to_owned()], detail: "This acquisition record is unreadable; its original content is preserved for recovery.".into() });
        }
        if is_source_index(path) {
            let source = self.root.join(path);
            let usable = read_page(&source).ok().is_some_and(|page| {
                self.source_dir(&page.info.source_id).ok().as_deref() == source.parent()
                    && self.open_source(&page.info.source_id).is_ok()
            });
            if !usable {
                diagnostics.push(CollectionDiagnostic { kind: "malformed_metadata".into(), paths: vec![path.to_owned()], detail: "Source metadata, identity, or acquisition records are unreadable or inconsistent; original files and retained assets remain preserved.".into() });
            }
        }
    }

    fn reconcile_snapshot(
        &mut self,
        files: BTreeMap<String, String>,
        mut diagnostics: Vec<CollectionDiagnostic>,
        candidates: BTreeMap<String, Vec<String>>,
        directory_mtimes: &BTreeMap<PathBuf, std::time::SystemTime>,
    ) -> Result<bool> {
        let ledger_path = self.root.join(".reconciliation/paths.md");
        let (previous, ledger_snapshot): (Ledger, Option<Vec<u8>>) = if ledger_path.exists() {
            let text = read_bounded_text(&ledger_path, MAX_PAGE_BYTES)?;
            let previous = serde_yaml_ng::from_str(
                text.strip_prefix("---\n")
                    .and_then(|v| v.split_once("\n---\n"))
                    .ok_or_else(|| {
                        GardenError::Invalid(
                            "The page location ledger is unreadable; original files are preserved."
                                .into(),
                        )
                    })?
                    .0,
            )?;
            (previous, Some(text.into_bytes()))
        } else {
            (Ledger::default(), None)
        };
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
        let mut bases = HashMap::new();
        for (path, fingerprint) in &files {
            let Ok(text) = read_bounded_text(&self.root.join(path), MAX_PAGE_BYTES) else {
                return Ok(false);
            };
            if hex::encode(Sha256::digest(text.as_bytes())) != *fingerprint {
                return Ok(false);
            }
            if path.starts_with("sources/") && !is_source_index(path) {
                continue;
            }
            // Never rewrite a damaged source record while reporting its gap.
            if is_source_index(path)
                && diagnostics.iter().any(|diagnostic| {
                    diagnostic.kind == "malformed_metadata"
                        && diagnostic.paths.iter().any(|item| item == path)
                })
            {
                continue;
            }
            let is_ambiguous = candidates
                .values()
                .any(|paths| paths.len() > 1 && paths.contains(path));
            if is_ambiguous
                || (path.starts_with("pages/")
                    && text.starts_with("---\n")
                    && parse_knowledge_header(&text).is_err())
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
            let rebased = map_markdown_links(&text, |href| {
                if resolve_relative(path, href).is_some_and(|target| files.contains_key(&target)) {
                    return href.to_owned();
                }
                rebase_href(origin, path, href)
            });
            let mut repaired = repair_links(path, &rebased, &aliases, &files, &mut diagnostics);
            if is_source_index(path) {
                if let Ok(mut page) = read_page(&self.root.join(path)) {
                    let mut changed = false;
                    for summary in &mut page.info.knowledge_pages {
                        if let Some(paths) = candidates
                            .get(&summary.page_id)
                            .filter(|paths| paths.len() == 1)
                        {
                            let header = read_knowledge_header(&self.root.join(&paths[0]))?;
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
            if repaired != text {
                bases.insert(self.root.join(path), text.into_bytes());
                writes.push((self.root.join(path), repaired.into_bytes()));
            }
        }
        let ledger_text = format!("---\n{}---\n\n# Page locations\n\nLast unambiguous paths, used to repair ordinary links after moves. Markdown pages remain authoritative.\n", serde_yaml_ng::to_string(&ledger)?);
        let ledger_bytes = ledger_snapshot.clone();
        if ledger_bytes.as_deref() != Some(ledger_text.as_bytes())
            && (!ledger.pages.is_empty() || ledger_bytes.is_some())
        {
            if let Some(bytes) = ledger_bytes {
                bases.insert(ledger_path.clone(), bytes);
            }
            writes.push((ledger_path.clone(), ledger_text.into_bytes()));
        }
        #[cfg(test)]
        if let Some(hook) = self.before_reconciliation_publication.take() {
            hook();
        }
        for (path, fingerprint) in &files {
            let Ok(text) = read_bounded_text(&self.root.join(path), MAX_PAGE_BYTES) else {
                return Ok(false);
            };
            if hex::encode(Sha256::digest(text.as_bytes())) != *fingerprint {
                return Ok(false);
            }
        }
        for diagnostic in &diagnostics {
            if diagnostic.kind == "unreadable_markdown"
                && diagnostic
                    .paths
                    .iter()
                    .any(|path| read_bounded_text(&self.root.join(path), MAX_PAGE_BYTES).is_ok())
            {
                return Ok(false);
            }
        }
        let ledger_unchanged = match (
            read_bounded_text(&ledger_path, MAX_PAGE_BYTES),
            ledger_snapshot.as_deref(),
        ) {
            (Ok(actual), Some(expected)) => actual.as_bytes() == expected,
            (Err(GardenError::Io(error)), None) => error.kind() == std::io::ErrorKind::NotFound,
            _ => false,
        };
        if !scan_directories_unchanged(directory_mtimes) || !ledger_unchanged {
            return Ok(false);
        }
        commit_publication(
            &self.root,
            writes,
            &bases,
            self.fail_after_publication_files.take(),
        )?;
        self.collection_diagnostics = diagnostics;
        Ok(true)
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
        let source_path = self
            .index
            .query_row(
                "SELECT summary FROM sources WHERE json_extract(summary, '$.page_id') = ?1",
                [page_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let from = if let Some(summary) = source_path {
            let summary: SourceSummary = serde_json::from_str(&summary)?;
            self.page_path(&summary.source_id)?
        } else {
            self.knowledge_path(page_id)?
        };
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
    map_markdown_links(text, |href| {
        let Some(target) = resolve_relative(from, href) else {
            return href.to_owned();
        };
        if let Some(destination) = aliases.get(&target) {
            if destination == &target {
                return href.to_owned();
            }
            let suffix = href.find(['#', '?']).map(|at| &href[at..]).unwrap_or("");
            return format!("{}{suffix}", relative_link(from, destination));
        }
        if target.ends_with(".md") && !files.contains_key(&target) {
            diagnostics.push(CollectionDiagnostic {
                kind: "unresolved_target".into(),
                paths: vec![from.into(), target],
                detail:
                    "The ordinary Markdown target is missing or ambiguous; the link was preserved."
                        .into(),
            });
        }
        href.to_owned()
    })
}

fn rebase_href(from: &str, to: &str, href: &str) -> String {
    let Some(target) = resolve_relative(from, href) else {
        return href.to_owned();
    };
    let suffix = href.find(['#', '?']).map(|at| &href[at..]).unwrap_or("");
    format!("{}{suffix}", relative_link(to, &target))
}

pub(super) fn rebase_links(from: &str, to: &str, text: &str) -> String {
    if from == to {
        return text.to_owned();
    }
    map_markdown_links(text, |href| rebase_href(from, to, href))
}

/// Rewrite only Markdown destinations, never YAML, fenced/indented code, or
/// inline code. Unsupported constructs remain unchanged and readable.
fn map_markdown_links(text: &str, mut rewrite: impl FnMut(&str) -> String) -> String {
    static LINKS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let links = LINKS.get_or_init(|| {
        regex::Regex::new(r"\]\((<[^>\n]+>|[^\s)]+)([^)]*)\)|(?m)^\[[^\]\n]+\]:\s*(<[^>\n]+>|\S+)")
            .unwrap()
    });
    let mut result = String::with_capacity(text.len());
    let mut yaml = text.starts_with("---\n");
    let mut fence = None::<(u8, usize)>;
    let mut ticks = None::<usize>;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if yaml {
            result.push_str(line);
            if index > 0 && line.trim_end() == "---" {
                yaml = false;
            }
            continue;
        }
        let trimmed = line.trim_start_matches(' ');
        let indent = line.len() - trimmed.len();
        let marker = trimmed.as_bytes().first().copied().unwrap_or(0);
        let run = trimmed.bytes().take_while(|byte| *byte == marker).count();
        if let Some((character, count)) = fence {
            result.push_str(line);
            if indent <= 3
                && marker == character
                && run >= count
                && trimmed[run..].trim().is_empty()
            {
                fence = None;
            }
            continue;
        }
        if indent <= 3 && matches!(marker, b'`' | b'~') && run >= 3 {
            fence = Some((marker, run));
            result.push_str(line);
            continue;
        }
        if indent >= 4 || line.starts_with('\t') {
            result.push_str(line);
            continue;
        }
        let mut start = 0;
        let mut at = 0;
        while at < line.len() {
            if line.as_bytes()[at] != b'`' {
                at += 1;
                continue;
            }
            let count = line.as_bytes()[at..]
                .iter()
                .take_while(|byte| **byte == b'`')
                .count();
            if ticks.is_none() {
                result.push_str(
                    &links.replace_all(&line[start..at], |capture: &regex::Captures<'_>| {
                        replace_destination(capture, &mut rewrite)
                    }),
                );
                start = at;
                ticks = Some(count);
            } else if ticks == Some(count) {
                result.push_str(&line[start..at + count]);
                start = at + count;
                ticks = None;
            }
            at += count;
        }
        if ticks.is_some() {
            result.push_str(&line[start..]);
        } else {
            result.push_str(
                &links.replace_all(&line[start..], |capture: &regex::Captures<'_>| {
                    replace_destination(capture, &mut rewrite)
                }),
            );
        }
    }
    result
}

fn replace_destination(
    capture: &regex::Captures<'_>,
    rewrite: &mut impl FnMut(&str) -> String,
) -> String {
    let matched = capture.get(1).or_else(|| capture.get(3)).unwrap();
    let raw = matched.as_str();
    let angle = raw.starts_with('<') && raw.ends_with('>');
    let href = if angle { &raw[1..raw.len() - 1] } else { raw };
    let replacement = rewrite(href);
    let replacement = if angle {
        format!("<{replacement}>")
    } else {
        replacement
    };
    let offset = matched.start() - capture.get(0).unwrap().start();
    let mut result = capture[0].to_owned();
    result.replace_range(offset..offset + raw.len(), &replacement);
    result
}

#[cfg(test)]
mod publication_race_tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn exercise_changed_read_dependency(add_page: bool) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let mut app = Application::open(&root).unwrap();
        let pages = root.join("pages");
        fs::create_dir_all(&pages).unwrap();
        let original = pages.join("owner.md");
        fs::write(&original, "original owner content").unwrap();
        let target = if add_page {
            pages.join("added.md")
        } else {
            original.clone()
        };
        let hook_called = Arc::new(AtomicBool::new(false));
        let called = Arc::clone(&hook_called);
        app.before_reconciliation_publication = Some(Box::new(move || {
            fs::write(target, "new owner content").unwrap();
            called.store(true, Ordering::SeqCst);
        }));
        let revision = app.collection_status().revision;
        for _ in 0..10 {
            let status = app.reconcile_collection(100).unwrap();
            if hook_called.load(Ordering::SeqCst) {
                assert!(
                    !status.complete,
                    "a changed read dependency must reject the snapshot"
                );
                assert_eq!(status.revision, revision);
                assert!(status.diagnostics.is_empty());
                break;
            }
        }
        assert!(hook_called.load(Ordering::SeqCst));
        for _ in 0..10 {
            let status = app.reconcile_collection(100).unwrap();
            if status.complete {
                let expected = if add_page { 2 } else { 1 };
                assert_eq!(
                    status
                        .diagnostics
                        .iter()
                        .filter(|item| item.kind == "malformed_metadata")
                        .count(),
                    expected
                );
                assert_eq!(
                    fs::read_to_string(if add_page {
                        pages.join("added.md")
                    } else {
                        original
                    })
                    .unwrap(),
                    "new owner content"
                );
                return;
            }
        }
        panic!("the stable snapshot should eventually reconcile");
    }

    #[test]
    fn changed_unwritten_page_rejects_the_snapshot() {
        exercise_changed_read_dependency(false);
    }

    #[test]
    fn added_page_rejects_the_snapshot() {
        exercise_changed_read_dependency(true);
    }

    #[test]
    fn newly_readable_page_rejects_the_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let mut app = Application::open(&root).unwrap();
        let page = root.join("pages/owner.md");
        fs::create_dir_all(page.parent().unwrap()).unwrap();
        fs::write(&page, [0xff]).unwrap();
        let target = page.clone();
        let hook_called = Arc::new(AtomicBool::new(false));
        let called = Arc::clone(&hook_called);
        app.before_reconciliation_publication = Some(Box::new(move || {
            fs::write(target, "now readable").unwrap();
            called.store(true, Ordering::SeqCst);
        }));
        for _ in 0..10 {
            let status = app.reconcile_collection(100).unwrap();
            if hook_called.load(Ordering::SeqCst) {
                assert!(
                    !status.complete,
                    "a newly readable page must reject the snapshot"
                );
                for _ in 0..10 {
                    let status = app.reconcile_collection(100).unwrap();
                    if status.complete {
                        assert!(status
                            .diagnostics
                            .iter()
                            .any(|item| item.kind == "malformed_metadata"
                                && item.paths.iter().any(|path| path == "pages/owner.md")));
                        assert!(!status
                            .diagnostics
                            .iter()
                            .any(|item| item.kind == "unreadable_markdown"));
                        return;
                    }
                }
                panic!("the readable page should be reconsidered");
            }
        }
        panic!("the publication hook should run");
    }

    #[test]
    fn oversized_ledger_edit_rejects_the_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let ledger = root.join(".reconciliation/paths.md");
        fs::create_dir_all(ledger.parent().unwrap()).unwrap();
        fs::write(&ledger, "---\npages: {}\n---\n\n# Locations\n").unwrap();
        let mut app = Application::open(&root).unwrap();
        let original_ledger = fs::read(&ledger).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::write(root.join("pages/owner.md"), "owner content").unwrap();
        let target = ledger.clone();
        let hook_called = Arc::new(AtomicBool::new(false));
        let called = Arc::clone(&hook_called);
        app.before_reconciliation_publication = Some(Box::new(move || {
            File::create(target)
                .unwrap()
                .set_len(MAX_PAGE_BYTES + 1)
                .unwrap();
            called.store(true, Ordering::SeqCst);
        }));
        for _ in 0..10 {
            let status = app.reconcile_collection(100).unwrap();
            if hook_called.load(Ordering::SeqCst) {
                assert!(
                    !status.complete,
                    "the oversized ledger edit must reject the snapshot"
                );
                fs::write(&ledger, original_ledger).unwrap();
                for _ in 0..10 {
                    if app.reconcile_collection(100).unwrap().complete {
                        return;
                    }
                }
                panic!("a restored ledger should allow reconciliation");
            }
        }
        panic!("the publication hook should run");
    }
}
