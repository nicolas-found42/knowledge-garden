//! Incremental candidate identity parsing; callers must authenticate the complete file.
use super::super::{GardenError, Result};
use std::{
    cell::Cell,
    io::{BufRead, BufReader, Read},
    rc::Rc,
};
use yaml_rust2::parser::{Event, Parser};
const MAX_EVENT_INPUT: usize = 256 * 1024;
const MAX_SCALAR: usize = 64 * 1024;
const MAX_DEPTH: usize = 32;

struct PositionedFile {
    file: std::fs::File,
    offset: u64,
}
impl Read for PositionedFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use std::os::unix::fs::FileExt;
        let n = self.file.read_at(buf, self.offset)?;
        self.offset += n as u64;
        Ok(n)
    }
}
struct HeaderReader<R> {
    input: BufReader<R>,
    line: Vec<u8>,
    pos: usize,
    first: bool,
    ended: bool,
    invalid: Rc<Cell<bool>>,
}
impl<R: Read> Read for HeaderReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() || self.ended {
            return Ok(0);
        }
        if self.pos == self.line.len() {
            self.line.clear();
            self.pos = 0;
            let n = self
                .input
                .by_ref()
                .take((MAX_SCALAR + 1) as u64)
                .read_until(b'\n', &mut self.line)?;
            if n == 0 || n > MAX_SCALAR {
                self.invalid.set(true);
                self.ended = true;
                return Ok(0);
            }
            if self.first {
                self.first = false;
                if self.line != b"---\n" {
                    self.invalid.set(true);
                    self.ended = true;
                    return Ok(0);
                }
            } else if self.line == b"---\n" {
                self.ended = true;
                return Ok(0);
            }
        }
        let n = out.len().min(self.line.len() - self.pos);
        out[..n].copy_from_slice(&self.line[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}
struct Utf8Chars<R> {
    input: R,
    budget: Rc<Cell<usize>>,
    invalid: Rc<Cell<bool>>,
}
impl<R: Read> Iterator for Utf8Chars<R> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        let mut bytes = [0; 4];
        match self.input.read(&mut bytes[..1]) {
            Ok(0) => return None,
            Ok(_) => (),
            Err(_) => {
                self.invalid.set(true);
                return None;
            }
        }
        let n = match bytes[0] {
            0..=0x7f => 1,
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => {
                self.invalid.set(true);
                return None;
            }
        };
        if self.budget.get().saturating_add(n) > MAX_EVENT_INPUT {
            self.invalid.set(true);
            return None;
        }
        if self.input.read_exact(&mut bytes[1..n]).is_err() {
            self.invalid.set(true);
            return None;
        }
        self.budget.set(self.budget.get() + n);
        match std::str::from_utf8(&bytes[..n])
            .ok()
            .and_then(|s| s.chars().next())
        {
            Some(c) => Some(c),
            None => {
                self.invalid.set(true);
                None
            }
        }
    }
}

/// Incremental header identity discovery from a caller-held descriptor.
/// The result is only a candidate: callers must authenticate the complete
/// retained file and reject duplicate identities before using it as authority.
pub(super) struct PageIdReader {
    parser: Parser<Utf8Chars<HeaderReader<PositionedFile>>>,
    invalid: Rc<Cell<bool>>,
    budget: Rc<Cell<usize>>,
    depth: usize,
    root_mapping: bool,
    expect_key: bool,
    key: Option<String>,
    keys: std::collections::HashSet<String>,
    page_id: Option<String>,
    done: bool,
}

impl PageIdReader {
    pub(super) fn new(file: &std::fs::File) -> Result<Self> {
        let invalid = Rc::new(Cell::new(false));
        let budget = Rc::new(Cell::new(0));
        let reader = HeaderReader {
            input: BufReader::new(PositionedFile {
                file: file.try_clone()?,
                offset: 0,
            }),
            line: Vec::new(),
            pos: 0,
            first: true,
            ended: false,
            invalid: invalid.clone(),
        };
        Ok(Self {
            parser: Parser::new(Utf8Chars {
                input: reader,
                budget: budget.clone(),
                invalid: invalid.clone(),
            }),
            invalid,
            budget,
            depth: 0,
            root_mapping: false,
            expect_key: true,
            key: None,
            keys: Default::default(),
            page_id: None,
            done: false,
        })
    }

    /// Advances at most `limit` parser events; None means more work remains.
    pub(super) fn advance(&mut self, limit: usize) -> Result<Option<String>> {
        if self.done {
            return Err(GardenError::Invalid(
                "Page identity reader already completed.".into(),
            ));
        }
        for _ in 0..limit.clamp(1, 128) {
            let (event, _) = self
                .parser
                .next_token()
                .map_err(|e| GardenError::Invalid(format!("Invalid page header: {e}")))?;
            if self.invalid.get() {
                return Err(GardenError::Invalid(
                    "Page header exceeds streaming limits or is invalid UTF-8.".into(),
                ));
            }
            self.budget.set(0);
            let mapping_start = matches!(&event, Event::MappingStart(..));
            match event {
                Event::Alias(_) => {
                    return Err(GardenError::Invalid(
                        "Page header aliases are unsupported.".into(),
                    ))
                }
                Event::Scalar(value, _, anchor, tag) => {
                    if value.len() > MAX_SCALAR || anchor != 0 || tag.is_some() {
                        return Err(GardenError::Invalid(
                            "Page header scalar, anchor or tag exceeds limits.".into(),
                        ));
                    }
                    if self.depth == 1 {
                        if self.expect_key {
                            if self.keys.len() >= 256 || !self.keys.insert(value.clone()) {
                                return Err(GardenError::Invalid(
                                    "Duplicate or excessive page header keys.".into(),
                                ));
                            }
                            self.key = Some(value);
                            self.expect_key = false;
                        } else {
                            if self.key.as_deref() == Some("page_id") {
                                if value.is_empty() || value.len() > 256 {
                                    return Err(GardenError::Invalid(
                                        "Invalid page identity.".into(),
                                    ));
                                }
                                self.page_id = Some(value);
                            }
                            self.expect_key = true;
                        }
                    }
                }
                Event::MappingStart(anchor, tag) | Event::SequenceStart(anchor, tag) => {
                    if anchor != 0 || tag.is_some() {
                        return Err(GardenError::Invalid(
                            "Page header anchors or tags are unsupported.".into(),
                        ));
                    }
                    if self.depth == 0 {
                        if !mapping_start || self.root_mapping {
                            return Err(GardenError::Invalid(
                                "Page header must have one root mapping.".into(),
                            ));
                        }
                        self.root_mapping = true;
                    } else if self.depth == 1 {
                        if self.expect_key || self.key.as_deref() == Some("page_id") {
                            return Err(GardenError::Invalid(
                                "Invalid page header key or identity.".into(),
                            ));
                        }
                    }
                    self.depth += 1;
                    if self.depth > MAX_DEPTH {
                        return Err(GardenError::Invalid(
                            "Page header nesting exceeds limits.".into(),
                        ));
                    }
                }
                Event::MappingEnd | Event::SequenceEnd => {
                    self.depth = self.depth.checked_sub(1).ok_or_else(|| {
                        GardenError::Invalid("Invalid page header nesting.".into())
                    })?;
                    if self.depth == 1 {
                        self.expect_key = true;
                    }
                }
                Event::StreamEnd => {
                    if !self.root_mapping || self.depth != 0 || !self.expect_key {
                        return Err(GardenError::Invalid("Page header is incomplete.".into()));
                    }
                    self.done = true;
                    return self
                        .page_id
                        .clone()
                        .map(Some)
                        .ok_or_else(|| GardenError::Invalid("Page identity is missing.".into()));
                }
                _ => (),
            }
        }
        Ok(None)
    }
}

/// Candidate identities are parsed only from a private, unlinked copy. The
/// copy is hashed as it is written, then compared to the retained snapshot.
pub(super) struct AuthenticatedCandidate {
    root: std::path::PathBuf,
    relative: std::path::PathBuf,
    expected: super::super::migration_backup::replay_authority::Expected,
    identity: (u64, u64, u64, i64, i64),
    source: std::fs::File,
    snapshot: std::fs::File,
    digest: Option<super::super::migration_backup::replay_authority::Digest>,
    copied: u64,
    verifier: Option<super::super::migration_backup::replay_authority::FileVerifier>,
    reader: Option<PageIdReader>,
    candidate: Option<String>,
    phase: u8,
}
fn identity(file: &std::fs::File) -> Result<(u64, u64, u64, i64, i64)> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    Ok((m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
}
impl AuthenticatedCandidate {
    pub(super) fn new(
        root: &std::path::Path,
        relative: &std::path::Path,
        expected: super::super::migration_backup::replay_authority::Expected,
    ) -> Result<Self> {
        let source = super::super::migration_backup::open_nofollow(root, relative)?;
        let identity = identity(&source)?;
        Ok(Self {
            root: root.into(),
            relative: relative.into(),
            expected,
            identity,
            source,
            snapshot: tempfile::tempfile()?,
            digest: Some(super::super::migration_backup::replay_authority::Digest::new(identity.2)),
            copied: 0,
            verifier: None,
            reader: None,
            candidate: None,
            phase: 0,
        })
    }
    pub(super) fn step(&mut self) -> Result<Option<String>> {
        use std::io::Write;
        if identity(&self.source)? != self.identity
            || identity(&super::super::migration_backup::open_nofollow(
                &self.root,
                &self.relative,
            )?)? != self.identity
        {
            return Err(GardenError::Recovery(
                "Page snapshot changed during identity discovery.".into(),
            ));
        }
        match self.phase {
            0 => {
                let count = (self.identity.2 - self.copied).min(64 * 1024) as usize;
                if count != 0 {
                    let mut block = vec![0; count];
                    self.source.read_exact(&mut block)?;
                    self.snapshot.write_all(&block)?;
                    for byte in block {
                        self.digest.as_mut().unwrap().push(byte);
                    }
                    self.copied += count as u64;
                }
                if self.copied == self.identity.2 {
                    self.digest.take().unwrap().verify(&self.expected)?;
                    self.reader = Some(PageIdReader::new(&self.snapshot)?);
                    self.phase = 1;
                }
            }
            1 => {
                if let Some(id) = self.reader.as_mut().unwrap().advance(1)? {
                    self.candidate = Some(id);
                    let verifier =
                        super::super::migration_backup::replay_authority::FileVerifier::new(
                            &self.root,
                            &self.relative,
                            self.expected.clone(),
                        )?;
                    if verifier.identity() != self.identity {
                        return Err(GardenError::Recovery(
                            "Page snapshot changed during identity discovery.".into(),
                        ));
                    }
                    self.verifier = Some(verifier);
                    self.phase = 2;
                }
            }
            2 => {
                if self.verifier.as_mut().unwrap().step()? {
                    self.phase = 3;
                    return Ok(self.candidate.take());
                }
            }
            _ => {
                return Err(GardenError::Recovery(
                    "Page identity discovery already completed.".into(),
                ))
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn candidate(text: &str) -> Result<String> {
        let mut file = tempfile::tempfile()?;
        file.write_all(text.as_bytes())?;
        let mut reader = PageIdReader::new(&file)?;
        loop {
            if let Some(id) = reader.advance(1)? {
                return Ok(id);
            }
        }
    }

    #[test]
    fn bounded_identity_parser_rejects_ambiguous_headers() {
        assert_eq!(
            candidate("---\npage_id: p1\nnested:\n  list: [one, two]\n---\nbody").unwrap(),
            "p1"
        );
        for header in [
            "---\npage_id: p1\npage_id: p2\n---\n",
            "---\npage_id: [p1]\n---\n",
            "---\npage_id: &id p1\nother: *id\n---\n",
            "---\npage_id: p1\n",
        ] {
            assert!(candidate(header).is_err());
        }
        assert!(candidate(&format!(
            "---\npage_id: {}\n---\n",
            "x".repeat(MAX_SCALAR + 1)
        ))
        .is_err());
    }
    #[test]
    fn identity_is_not_returned_for_changed_snapshot_bytes() {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        let original = b"---\npage_id: p1\n---\nbody";
        std::fs::write(root.path().join("page.md"), original).unwrap();
        let expected = super::super::super::migration_backup::replay_authority::Expected::Legacy(
            hex::encode(Sha256::digest(original)),
        );
        let mut candidate =
            AuthenticatedCandidate::new(root.path(), std::path::Path::new("page.md"), expected)
                .unwrap();
        assert!(candidate.step().unwrap().is_none());
        std::fs::write(root.path().join("page.md"), b"---\npage_id: p2\n---\nbody").unwrap();
        loop {
            match candidate.step() {
                Err(_) => break,
                Ok(Some(_)) => panic!("changed snapshot was accepted"),
                Ok(None) => (),
            }
        }
    }
    #[test]
    fn verified_private_copy_yields_identity_in_bounded_turns() {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        let mut original = b"---\npage_id: retained-id\n---\n".to_vec();
        original.extend(vec![b'x'; 3 * 64 * 1024]);
        std::fs::write(root.path().join("page.md"), &original).unwrap();
        let expected = super::super::super::migration_backup::replay_authority::Expected::Legacy(
            hex::encode(Sha256::digest(&original)),
        );
        let mut candidate =
            AuthenticatedCandidate::new(root.path(), std::path::Path::new("page.md"), expected)
                .unwrap();
        let mut turns = 0;
        let found = loop {
            turns += 1;
            if let Some(id) = candidate.step().unwrap() {
                break id;
            }
        };
        assert_eq!(found, "retained-id");
        assert!(turns >= 8);
    }
    #[test]
    fn resolver_restarts_and_rejects_duplicate_snapshot_ids() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pages")).unwrap();
        std::fs::write(
            root.path().join("pages/moved.md"),
            b"---\npage_id: page-1\n---\nbody",
        )
        .unwrap();
        super::super::super::migration_backup::prepare_pre_recovery(root.path()).unwrap();
        let mut first = Resolver::new(root.path(), "page-1".into(), false).unwrap();
        for _ in 0..3 {
            assert!(first.step(root.path()).unwrap().is_none());
        }
        drop(first);
        let mut resumed = Resolver::new(root.path(), "page-1".into(), false).unwrap();
        let found = loop {
            if let Some(value) = resumed.step(root.path()).unwrap() {
                break value;
            }
        };
        assert_eq!(found, Some(std::path::PathBuf::from("pages/moved.md")));

        let duplicate = tempfile::tempdir().unwrap();
        std::fs::create_dir(duplicate.path().join("pages")).unwrap();
        for name in ["a.md", "b.md"] {
            std::fs::write(
                duplicate.path().join("pages").join(name),
                b"---\npage_id: page-1\n---\nbody",
            )
            .unwrap();
        }
        super::super::super::migration_backup::prepare_pre_recovery(duplicate.path()).unwrap();
        let mut resolver = Resolver::new(duplicate.path(), "page-1".into(), false).unwrap();
        loop {
            match resolver.step(duplicate.path()) {
                Err(_) => break,
                Ok(Some(_)) => panic!("duplicate accepted"),
                Ok(None) => (),
            }
        }
    }
    #[test]
    fn resolver_fences_live_page_after_snapshot() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pages")).unwrap();
        let page = root.path().join("pages/moved.md");
        std::fs::write(&page, b"---\npage_id: page-1\n---\nbody").unwrap();
        super::super::super::migration_backup::prepare_pre_recovery(root.path()).unwrap();
        std::fs::write(&page, b"---\npage_id: page-1\n---\nedit").unwrap();
        let mut resolver = Resolver::new(root.path(), "page-1".into(), false).unwrap();
        loop {
            match resolver.step(root.path()) {
                Err(_) => break,
                Ok(Some(_)) => panic!("changed live page accepted"),
                Ok(None) => (),
            }
        }
        assert!(std::fs::read(&page).unwrap().ends_with(b"edit"));
    }
    #[test]
    fn resolver_binds_trusted_record_and_fences_its_live_bytes() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pages")).unwrap();
        let body = b"---\npage_id: page-1\n---\nbody";
        std::fs::write(root.path().join("pages/moved.md"), body).unwrap();
        super::super::super::trusted_pages::record(
            root.path(),
            std::path::Path::new("pages/moved.md"),
            body,
        )
        .unwrap();
        super::super::super::migration_backup::prepare_pre_recovery(root.path()).unwrap();
        let records = root.path().join(".recovery/trusted-pages/records");
        let record = std::fs::read_dir(records)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(record, b"{}").unwrap();
        let mut resolver = Resolver::new(root.path(), "page-1".into(), true).unwrap();
        loop {
            match resolver.step(root.path()) {
                Err(_) => break,
                Ok(Some(_)) => panic!("changed trusted record accepted"),
                Ok(None) => (),
            }
        }
    }
    #[test]
    fn resolver_rejects_a_changed_live_trusted_blob() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pages")).unwrap();
        let body = b"---\npage_id: page-1\n---\nbody";
        std::fs::write(root.path().join("pages/moved.md"), body).unwrap();
        super::super::super::trusted_pages::record(
            root.path(),
            std::path::Path::new("pages/moved.md"),
            body,
        )
        .unwrap();
        super::super::super::migration_backup::prepare_pre_recovery(root.path()).unwrap();
        let blobs = root.path().join(".recovery/trusted-pages/blobs");
        let blob = std::fs::read_dir(blobs)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(blob, b"tampered").unwrap();
        let mut resolver = Resolver::new(root.path(), "page-1".into(), true).unwrap();
        loop {
            match resolver.step(root.path()) {
                Err(_) => break,
                Ok(Some(_)) => panic!("changed trusted blob accepted"),
                Ok(None) => (),
            }
        }
    }
    #[test]
    fn published_output_fence_preserves_conflicting_external_save() {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pages")).unwrap();
        let page = root.path().join("pages/moved.md");
        std::fs::write(&page, b"---\npage_id: page-1\n---\nbase").unwrap();
        std::fs::write(
            root.path().join("proposed"),
            b"---\npage_id: page-1\n---\nnew",
        )
        .unwrap();
        super::super::super::migration_backup::prepare_pre_recovery(root.path()).unwrap();
        std::fs::write(&page, b"---\npage_id: page-1\n---\nexternal").unwrap();
        let digest = super::super::super::migration_backup::replay_authority::Expected::Legacy(
            hex::encode(Sha256::digest(b"---\npage_id: page-1\n---\nnew")),
        );
        let mut resolver = Resolver::new(root.path(), "page-1".into(), false).unwrap();
        resolver.permit_published_output("proposed".into(), digest, "pages/moved.md".into());
        loop {
            match resolver.step(root.path()) {
                Err(_) => break,
                Ok(Some(_)) => panic!("external edit accepted as publication"),
                Ok(None) => (),
            }
        }
        assert!(std::fs::read(&page).unwrap().ends_with(b"external"));
    }
    #[test]
    fn resolver_discovers_new_authenticated_overlay_records() {
        use sha2::{Digest, Sha256};
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("pages")).unwrap();
        super::super::super::migration_backup::prepare_pre_recovery(root.path()).unwrap();
        let page = b"---\npage_id: overlay-id\n---\nbody";
        super::super::super::trusted_pages::record(
            root.path(),
            std::path::Path::new("pages/new.md"),
            page,
        )
        .unwrap();
        let name = hex::encode(Sha256::digest(b"pages/new.md"));
        let relative = std::path::Path::new(".recovery/trusted-pages/records").join(name);
        let bytes = std::fs::read(root.path().join(&relative)).unwrap();
        let overlay = RecordOverlay::new().unwrap();
        overlay.put(&relative, &bytes).unwrap();
        let mut resolver = Resolver::new(root.path(), "overlay-id".into(), true).unwrap();
        let result = loop {
            if let Some(result) = resolver
                .step_with_overlay(root.path(), Some(&overlay))
                .unwrap()
            {
                break result;
            }
        };
        assert_eq!(result, Some(std::path::PathBuf::from("pages/new.md")));
    }
}

#[derive(serde::Deserialize)]
struct SnapshotRecord {
    path: std::path::PathBuf,
    sha256: String,
}

/// Scans one authenticated snapshot row or one bounded file chunk per turn.
/// A result is provisional until the matched live record passes its fence.
pub(super) struct Resolver {
    rows: super::super::migration_backup::replay_authority::SnapshotRows,
    wanted: String,
    trusted: bool,
    candidate: Option<(
        std::path::PathBuf,
        std::path::PathBuf,
        super::super::migration_backup::replay_authority::Expected,
        Option<(
            std::path::PathBuf,
            super::super::migration_backup::replay_authority::Expected,
        )>,
        AuthenticatedCandidate,
    )>,
    found: Option<(
        std::path::PathBuf,
        std::path::PathBuf,
        super::super::migration_backup::replay_authority::Expected,
        Option<(
            std::path::PathBuf,
            super::super::migration_backup::replay_authority::Expected,
        )>,
    )>,
    fence: Option<super::super::migration_backup::replay_authority::FileVerifier>,
    scanned: bool,
    overlay_after: String,
    overlay_done: bool,
    record_fenced: bool,
    output: Option<(
        std::path::PathBuf,
        super::super::migration_backup::replay_authority::Expected,
        String,
    )>,
    output_fence: bool,
}
impl Resolver {
    #[cfg(test)]
    pub(super) fn new(root: &std::path::Path, wanted: String, trusted: bool) -> Result<Self> {
        Self::new_with_index(root, wanted, trusted, None)
    }
    pub(super) fn new_with_index(
        root: &std::path::Path,
        wanted: String,
        trusted: bool,
        verified: Option<
            std::sync::Arc<super::super::migration_backup::verified_index::VerifiedIndex>,
        >,
    ) -> Result<Self> {
        Ok(Self {
            rows: super::super::migration_backup::replay_authority::SnapshotRows::new(
                root, verified,
            )?,
            wanted,
            trusted,
            candidate: None,
            found: None,
            fence: None,
            scanned: false,
            overlay_after: String::new(),
            overlay_done: false,
            record_fenced: false,
            output: None,
            output_fence: false,
        })
    }
    fn enqueue_record(
        &mut self,
        root: &std::path::Path,
        path: std::path::PathBuf,
        bytes: Vec<u8>,
        replaced: bool,
    ) -> Result<()> {
        use super::super::migration_backup::replay_authority::Expected;
        use sha2::{Digest, Sha256};
        let record: SnapshotRecord = serde_json::from_slice(&bytes)?;
        super::super::validate_relative_collection_path(&record.path)?;
        let filename = hex::encode(Sha256::digest(record.path.to_string_lossy().as_bytes()));
        if path.parent() != Some(std::path::Path::new(".recovery/trusted-pages/records"))
            || path.file_name().and_then(|s| s.to_str()) != Some(filename.as_str())
            || record.sha256.len() != 64
            || !record
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(GardenError::Recovery(
                "Trusted page record is invalid.".into(),
            ));
        }
        if !record.path.starts_with("pages") || record.path.extension().is_none_or(|x| x != "md") {
            return Ok(());
        }
        let blob = std::path::Path::new(".recovery/trusted-pages/blobs").join(&record.sha256);
        let source = if replaced {
            blob.clone()
        } else {
            self.rows.file(&blob)
        };
        let candidate =
            AuthenticatedCandidate::new(root, &source, Expected::Legacy(record.sha256.clone()))?;
        self.candidate = Some((
            record.path,
            path,
            Expected::Legacy(hex::encode(Sha256::digest(bytes))),
            Some((blob, Expected::Legacy(record.sha256))),
            candidate,
        ));
        Ok(())
    }
    pub(super) fn permit_published_output(
        &mut self,
        staged: std::path::PathBuf,
        digest: super::super::migration_backup::replay_authority::Expected,
        original_destination: String,
    ) {
        self.output = Some((staged, digest, original_destination));
    }
    #[cfg(test)]
    pub(super) fn step(
        &mut self,
        root: &std::path::Path,
    ) -> Result<Option<Option<std::path::PathBuf>>> {
        self.step_with_overlay(root, None)
    }
    pub(super) fn step_with_overlay(
        &mut self,
        root: &std::path::Path,
        overlay: Option<&RecordOverlay>,
    ) -> Result<Option<Option<std::path::PathBuf>>> {
        use super::super::migration_backup::replay_authority::Expected;
        use sha2::{Digest, Sha256};
        if let Some((path, fence_path, fence_expected, blob, reader)) = self.candidate.as_mut() {
            if let Some(id) = reader.step()? {
                if id == self.wanted {
                    if self.found.is_some() {
                        return Err(GardenError::Recovery(
                            "Duplicate page identities in the recovery snapshot.".into(),
                        ));
                    }
                    self.found = Some((
                        path.clone(),
                        fence_path.clone(),
                        fence_expected.clone(),
                        blob.clone(),
                    ));
                }
                self.candidate = None;
            }
            return Ok(None);
        }
        if !self.scanned {
            let Some((path, digest)) = self.rows.next(root)? else {
                self.scanned = true;
                return Ok(None);
            };
            let Some(digest) = digest else {
                return Ok(None);
            };
            if self.trusted {
                if path.parent() != Some(std::path::Path::new(".recovery/trusted-pages/records")) {
                    return Ok(None);
                }
                if overlay
                    .map(|index| index.get(&path))
                    .transpose()?
                    .flatten()
                    .is_some()
                {
                    return Ok(None);
                }
                let bytes = super::super::migration_backup::read_bounded_nofollow(
                    root,
                    &self.rows.file(&path),
                    16384,
                )?;
                digest.verify_bytes(&bytes)?;
                self.enqueue_record(root, path, bytes, false)?;
            } else if path.starts_with("pages") && path.extension().is_some_and(|x| x == "md") {
                let candidate =
                    AuthenticatedCandidate::new(root, &self.rows.file(&path), digest.clone())?;
                self.candidate = Some((path.clone(), path, digest, None, candidate));
            }
            return Ok(None);
        }
        if self.trusted && !self.overlay_done {
            if let Some((path, bytes)) = overlay
                .map(|index| index.next(&self.overlay_after))
                .transpose()?
                .flatten()
            {
                self.overlay_after = path.to_string_lossy().into_owned();
                self.enqueue_record(root, path, bytes, true)?;
                return Ok(None);
            }
            self.overlay_done = true;
            return Ok(None);
        }
        let Some((path, fence_path, expected, blob)) = &self.found else {
            let Some((_, digest, original_destination)) = &self.output else {
                return Ok(Some(None));
            };
            let path = std::path::Path::new(original_destination);
            match std::fs::symlink_metadata(root.join(path)) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(Some(None))
                }
                Err(error) => return Err(error.into()),
                Ok(metadata) if metadata.file_type().is_file() => (),
                Ok(_) => {
                    return Err(GardenError::Recovery(
                        "New page destination is not a regular file.".into(),
                    ))
                }
            }
            if self.fence.is_none() {
                self.fence = Some(
                    super::super::migration_backup::replay_authority::FileVerifier::new(
                        root,
                        path,
                        digest.clone(),
                    )?,
                );
                return Ok(None);
            }
            if let Some(matches) = self.fence.as_mut().unwrap().step_match()? {
                if !matches {
                    return Err(GardenError::Recovery(
                        "New page destination contains an external save.".into(),
                    ));
                }
                return Ok(Some(Some(path.to_owned())));
            }
            return Ok(None);
        };
        let (fence_path, expected) = if self.record_fenced {
            match blob {
                Some((path, expected)) => (path, expected),
                None => return Ok(Some(Some(path.clone()))),
            }
        } else {
            (fence_path, expected)
        };
        if self.fence.is_none() {
            self.fence = Some(
                super::super::migration_backup::replay_authority::FileVerifier::new(
                    root,
                    fence_path,
                    expected.clone(),
                )?,
            );
            return Ok(None);
        }
        if let Some(matches) = self.fence.as_mut().unwrap().step_match()? {
            self.fence = None;
            if !matches {
                if self.trusted || self.output_fence {
                    return Err(GardenError::Recovery(
                        "Live page or trusted evidence changed during replay.".into(),
                    ));
                }
                let (staged, digest, original_destination) =
                    self.output.as_ref().ok_or_else(|| {
                        GardenError::Recovery("Live page changed during replay.".into())
                    })?;
                let expected_output = if path == std::path::Path::new(original_destination) {
                    digest.clone()
                } else {
                    let bytes = super::super::migration_backup::read_bounded_nofollow(
                        root,
                        staged,
                        4 * 1024 * 1024,
                    )?;
                    digest.verify_bytes(&bytes)?;
                    let text = std::str::from_utf8(&bytes).map_err(|_| {
                        GardenError::Recovery("Published page is not UTF-8.".into())
                    })?;
                    let rebased = super::super::reconciliation::rebase_links(
                        original_destination,
                        &path.to_string_lossy(),
                        text,
                    );
                    Expected::Legacy(hex::encode(Sha256::digest(rebased.as_bytes())))
                };
                self.fence = Some(
                    super::super::migration_backup::replay_authority::FileVerifier::new(
                        root,
                        path,
                        expected_output,
                    )?,
                );
                self.output_fence = true;
                return Ok(None);
            }
            if self.record_fenced || self.output_fence {
                return Ok(Some(Some(path.clone())));
            }
            self.record_fenced = true;
        }
        Ok(None)
    }
}

/// Process-local authenticated cache of record outputs already replayed in
/// this transaction. Reconstructed by replaying prior entries after restart.
pub(super) struct RecordOverlay {
    _work: tempfile::TempDir,
    db: rusqlite::Connection,
    key: [u8; 32],
}
impl RecordOverlay {
    pub(super) fn new() -> Result<Self> {
        let work = tempfile::tempdir()?;
        let db = rusqlite::Connection::open(work.path().join("records.sqlite"))?;
        db.execute_batch("CREATE TABLE records (path TEXT PRIMARY KEY, bytes BLOB NOT NULL, tag BLOB NOT NULL) WITHOUT ROWID")?;
        let mut key = [0; 32];
        key[..16].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        key[16..].copy_from_slice(uuid::Uuid::new_v4().as_bytes());
        Ok(Self {
            _work: work,
            db,
            key,
        })
    }
    fn tag(&self, path: &str, bytes: &[u8]) -> [u8; 32] {
        let mut mac = super::replay_manifest::Mac::new(&self.key, "trusted-record-overlay", 0);
        mac.update(&(path.len() as u64).to_be_bytes());
        mac.update(path.as_bytes());
        mac.update(bytes);
        mac.finish()
    }
    pub(super) fn put(&self, path: &std::path::Path, bytes: &[u8]) -> Result<()> {
        let path = path
            .to_str()
            .ok_or_else(|| GardenError::Recovery("Trusted record path is invalid.".into()))?;
        let tag = self.tag(path, bytes);
        self.db.execute("INSERT INTO records(path,bytes,tag) VALUES (?1,?2,?3) ON CONFLICT(path) DO UPDATE SET bytes=excluded.bytes, tag=excluded.tag", rusqlite::params![path, bytes, tag.as_slice()])?;
        Ok(())
    }
    fn next(&self, after: &str) -> Result<Option<(std::path::PathBuf, Vec<u8>)>> {
        use rusqlite::OptionalExtension;
        let path: Option<String> = self.db.query_row("SELECT CASE WHEN length(path)<=16384 THEN path ELSE '' END FROM records WHERE path>?1 ORDER BY path LIMIT 1", [after], |row| row.get(0)).optional()?;
        let Some(path) = path else {
            return Ok(None);
        };
        if path.is_empty() {
            return Err(GardenError::Recovery(
                "Trusted record overlay path is invalid.".into(),
            ));
        }
        let bytes = self.get(std::path::Path::new(&path))?.ok_or_else(|| {
            GardenError::Recovery("Trusted record overlay changed during discovery.".into())
        })?;
        Ok(Some((path.into(), bytes)))
    }
    fn get(&self, path: &std::path::Path) -> Result<Option<Vec<u8>>> {
        use rusqlite::OptionalExtension;
        let path = path
            .to_str()
            .ok_or_else(|| GardenError::Recovery("Trusted record path is invalid.".into()))?;
        let row: Option<(Vec<u8>, Vec<u8>)> = self.db.query_row("SELECT CASE WHEN length(bytes)<=16384 THEN bytes ELSE x'' END,CASE WHEN length(tag)=32 THEN tag ELSE x'' END FROM records WHERE path=?1", [path], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
        match row {
            None => Ok(None),
            Some((bytes, tag)) if !bytes.is_empty() && self.tag(path, &bytes).as_slice() == tag => {
                Ok(Some(bytes))
            }
            Some(_) => Err(GardenError::Recovery(
                "Trusted record replay overlay changed.".into(),
            )),
        }
    }
}
