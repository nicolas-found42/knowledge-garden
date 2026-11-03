//! Frame legacy publication records in fixed-size reads before replay. Scratch
//! files are private and disposable: after a crash the manifest is scanned again.
use super::*;
use std::io::{BufReader, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;

const BUDGET: usize = 64 * 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Prefix,
    EntryFirst,
    EntryRequired,
    Entry,
    EntrySeparator,
    AfterEntries,
    RemovalPrefix,
    RemovalFirst,
    RemovalRequired,
    Removal,
    RemovalSeparator,
    Tail,
    Done,
}
pub(super) struct Scanner {
    root: PathBuf,
    relative: PathBuf,
    input: BufReader<File>,
    identity: (u64, u64, u64, i64, i64),
    work: PathBuf,
    phase: Phase,
    pos: usize,
    quoted: bool,
    depth: usize,
    in_string: bool,
    escaped: bool,
    output: Option<ScratchOutput>,
    entries: u64,
    removals: u64,
    cleaned_entries: u64,
    cleaned_removals: u64,
    cleaned_bases: u64,
    cleaned_metadata: u64,
    cleaned_directories: u8,
    digest: Option<migration_backup::replay_authority::Digest>,
    decoder: Option<super::replay_entry::Decoder>,
    decoded: u64,
    _auth_work: tempfile::TempDir,
    auth: Connection,
    auth_key: [u8; 32],
    record_mac: Option<Mac>,
}
pub(super) struct Mac {
    inner: Sha256,
    outer: [u8; 64],
}
impl Mac {
    pub(super) fn new(key: &[u8; 32], domain: &str, index: u64) -> Self {
        let mut inner_pad = [0x36; 64];
        let mut outer = [0x5c; 64];
        for i in 0..key.len() {
            inner_pad[i] ^= key[i];
            outer[i] ^= key[i];
        }
        let mut inner = Sha256::new();
        inner.update(inner_pad);
        inner.update(b"knowledge-garden-replay-scratch-v1\0");
        inner.update(domain.as_bytes());
        inner.update(index.to_be_bytes());
        Self { inner, outer }
    }
    pub(super) fn update(&mut self, bytes: &[u8]) {
        self.inner.update(bytes);
    }
    pub(super) fn finish(self) -> [u8; 32] {
        let mut outer = Sha256::new();
        outer.update(self.outer);
        outer.update(self.inner.finalize());
        outer.finalize().into()
    }
}
fn identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    )
}
fn create_scratch(root: &Path, work: &Path) -> Result<()> {
    use rustix::fs::{mkdirat, Mode};
    let (base, _) = replay_parent(
        root,
        &Path::new(".recovery/migration/replay-scratch").join("placeholder"),
        true,
    )?;
    let metadata = File::from(base).metadata()?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(GardenError::Recovery(
            "Replay scratch parent is not private.".into(),
        ));
    }
    let generation = work
        .parent()
        .ok_or_else(|| GardenError::Recovery("Replay scratch generation is missing.".into()))?;
    let (generation_dir, _) = replay_parent(root, &generation.join("placeholder"), true)?;
    let metadata = File::from(generation_dir).metadata()?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(GardenError::Recovery(
            "Replay scratch generation is not private.".into(),
        ));
    }
    let (directory, _) = replay_parent(root, &work.join("placeholder"), true)?;
    let metadata = File::from(directory.try_clone().map_err(std::io::Error::from)?).metadata()?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(GardenError::Recovery(
            "Replay scratch directory is not private.".into(),
        ));
    }
    for name in ["entries", "removals", "bases", "metadata"] {
        match mkdirat(&directory, name, Mode::from_bits_truncate(0o700)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => (),
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        let (child, _) = replay_parent(root, &work.join(name).join("placeholder"), false)?;
        let metadata = File::from(child).metadata()?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err(GardenError::Recovery(
                "Replay scratch directory is not private.".into(),
            ));
        }
    }
    File::from(directory).sync_all()?;
    Ok(())
}
pub(super) struct ScratchOutput {
    file: File,
    root: PathBuf,
    path: PathBuf,
    inode: (u64, u64),
    offset: u64,
    buffer: Vec<u8>,
}
impl ScratchOutput {
    pub(super) fn open(root: &Path, path: &Path) -> Result<Self> {
        use rustix::fs::{openat, Mode, OFlags};
        use rustix::io::Errno;
        let (parent, name) = replay_parent(root, path, false)?;
        let flags =
            OFlags::RDWR | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let fd = match openat(
            &parent,
            &name,
            flags | OFlags::CREATE | OFlags::EXCL,
            Mode::from_bits_truncate(0o600),
        ) {
            Ok(fd) => {
                File::from(parent.try_clone().map_err(std::io::Error::from)?).sync_all()?;
                fd
            }
            Err(Errno::EXIST) => {
                openat(&parent, &name, flags, Mode::empty()).map_err(std::io::Error::from)?
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        let file = File::from(fd);
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(GardenError::Recovery(
                "Replay scratch entry is not a regular file.".into(),
            ));
        }
        Ok(Self {
            file,
            root: root.into(),
            path: path.into(),
            inode: (metadata.dev(), metadata.ino()),
            offset: 0,
            buffer: Vec::with_capacity(BUDGET),
        })
    }
    pub(super) fn push(&mut self, byte: u8) {
        self.buffer.push(byte);
    }
    fn check(&self) -> Result<()> {
        let held = self.file.metadata()?;
        let current = migration_backup::open_nofollow(&self.root, &self.path)?.metadata()?;
        if !held.is_file()
            || held.nlink() != 1
            || current.nlink() != 1
            || (held.dev(), held.ino()) != self.inode
            || (current.dev(), current.ino()) != self.inode
        {
            return Err(GardenError::Recovery(
                "Replay scratch entry was replaced or linked.".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.check()?;
        let length = self.file.metadata()?.len();
        if self.offset > length {
            return Err(GardenError::Recovery(
                "Replay scratch was truncated.".into(),
            ));
        }
        let existing = (length - self.offset).min(self.buffer.len() as u64) as usize;
        self.file.seek(SeekFrom::Start(self.offset))?;
        let mut prior = vec![0; existing];
        self.file.read_exact(&mut prior)?;
        if prior != self.buffer[..existing] {
            return Err(GardenError::Recovery(
                "Replay scratch conflicts with retained manifest bytes.".into(),
            ));
        }
        self.file.write_all(&self.buffer[existing..])?;
        self.file.sync_all()?;
        self.check()?;
        self.offset += self.buffer.len() as u64;
        self.buffer.clear();
        Ok(())
    }
    pub(super) fn finish(mut self) -> Result<()> {
        self.flush()?;
        if self.file.metadata()?.len() != self.offset {
            return Err(GardenError::Recovery(
                "Replay scratch has unexpected trailing bytes.".into(),
            ));
        }
        Ok(())
    }
}
fn remove_scratch(root: &Path, path: &Path, directory: bool) -> Result<()> {
    use rustix::fs::{openat, unlinkat, AtFlags, Mode, OFlags};
    let (parent, name) = replay_parent(root, path, false)?;
    if !directory {
        let fd = openat(
            &parent,
            &name,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        if !File::from(fd).metadata()?.is_file() {
            return Err(GardenError::Recovery(
                "Replay scratch entry is not a regular file.".into(),
            ));
        }
    }
    unlinkat(
        &parent,
        &name,
        if directory {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        },
    )
    .map_err(std::io::Error::from)?;
    File::from(parent).sync_all()?;
    Ok(())
}
impl Scanner {
    pub(super) fn new(root: &Path, transaction: &Path, generation: &str) -> Result<Self> {
        let relative = transaction
            .strip_prefix(root)
            .map_err(|_| {
                GardenError::Recovery("Publication transaction escaped the collection.".into())
            })?
            .join("manifest.json");
        let file = migration_backup::open_nofollow(root, &relative)?;
        let identity = identity(&file.metadata()?);
        if !generation.starts_with("pre-work-")
            || Path::new(generation).components().count() != 1
            || generation.len() > 128
        {
            return Err(GardenError::Recovery(
                "Replay scratch generation is invalid.".into(),
            ));
        }
        let key = hex::encode(Sha256::digest(serde_json::to_vec(&relative)?));
        let work = PathBuf::from(".recovery/migration/replay-scratch")
            .join(generation)
            .join(key);
        create_scratch(root, &work)?;
        let origin_path = work.join("origin.json");
        let origin_bytes =
            serde_json::to_vec(&serde_json::json!({"schema":1,"manifest":relative}))?;
        let mut origin = ScratchOutput::open(root, &origin_path)?;
        for byte in origin_bytes {
            origin.push(byte);
        }
        origin.finish()?;
        let auth_work = tempfile::tempdir()?;
        let auth = Connection::open(auth_work.path().join("auth.sqlite"))?;
        auth.execute_batch("CREATE TABLE tags (kind TEXT NOT NULL, item INTEGER NOT NULL, tag BLOB NOT NULL, PRIMARY KEY(kind,item)) WITHOUT ROWID")?;
        let mut auth_key = [0u8; 32];
        auth_key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        auth_key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        Ok(Self {
            root: root.into(),
            relative,
            input: BufReader::new(file),
            identity,
            work,
            phase: Phase::Prefix,
            pos: 0,
            quoted: false,
            depth: 0,
            in_string: false,
            escaped: false,
            output: None,
            entries: 0,
            removals: 0,
            cleaned_entries: 0,
            cleaned_removals: 0,
            cleaned_bases: 0,
            cleaned_metadata: 0,
            cleaned_directories: 0,
            digest: Some(migration_backup::replay_authority::Digest::new(identity.2)),
            decoder: None,
            decoded: 0,
            _auth_work: auth_work,
            auth,
            auth_key,
            record_mac: None,
        })
    }
    pub(super) fn entry_count(&self) -> u64 {
        self.entries
    }
    pub(super) fn removal_count(&self) -> u64 {
        self.removals
    }
    pub(super) fn cleanup_step(&mut self) -> Result<bool> {
        if self.cleaned_entries < self.entries {
            remove_scratch(
                &self.root,
                &self
                    .work
                    .join("entries")
                    .join(self.cleaned_entries.to_string()),
                false,
            )?;
            self.cleaned_entries += 1;
            return Ok(false);
        }
        if self.cleaned_removals < self.removals {
            remove_scratch(
                &self.root,
                &self
                    .work
                    .join("removals")
                    .join(self.cleaned_removals.to_string()),
                false,
            )?;
            self.cleaned_removals += 1;
            return Ok(false);
        }
        if self.cleaned_bases < self.entries {
            let path = self.work.join("bases").join(self.cleaned_bases.to_string());
            match fs::symlink_metadata(self.root.join(&path)) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
                Ok(_) => remove_scratch(&self.root, &path, false)?,
            }
            self.cleaned_bases += 1;
            return Ok(false);
        }
        if self.cleaned_metadata < self.entries {
            remove_scratch(
                &self.root,
                &self
                    .work
                    .join("metadata")
                    .join(self.cleaned_metadata.to_string()),
                false,
            )?;
            self.cleaned_metadata += 1;
            return Ok(false);
        }
        let (path, directory) = match self.cleaned_directories {
            0 => (self.work.join("origin.json"), false),
            1 => (self.work.join("entries"), true),
            2 => (self.work.join("removals"), true),
            3 => (self.work.join("bases"), true),
            4 => (self.work.join("metadata"), true),
            5 => (self.work.clone(), true),
            _ => return Ok(true),
        };
        remove_scratch(&self.root, &path, directory)?;
        self.cleaned_directories += 1;
        Ok(false)
    }
    pub(super) fn entry(&self, index: u64) -> Result<super::replay_entry::Meta> {
        let bytes = migration_backup::read_bounded_nofollow(
            &self.root,
            &self.work.join("metadata").join(index.to_string()),
            131072,
        )?;
        self.verify_tag("metadata", index, &bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub(super) fn base(&self, index: u64, length: u64) -> Result<Vec<u8>> {
        migration_backup::read_bounded_nofollow(
            &self.root,
            &self.work.join("bases").join(index.to_string()),
            length,
        )
    }
    pub(super) fn removal(&self, index: u64) -> Result<String> {
        let bytes = migration_backup::read_bounded_nofollow(
            &self.root,
            &self.work.join("removals").join(index.to_string()),
            131072,
        )?;
        self.verify_tag("removals", index, &bytes)?;
        Ok(serde_json::from_slice(&bytes)?)
    }
    fn store_tag(&self, kind: &str, index: u64, tag: &[u8]) -> Result<()> {
        let index = i64::try_from(index)
            .map_err(|_| GardenError::Recovery("Replay scratch index is too large.".into()))?;
        self.auth.execute(
            "INSERT INTO tags(kind,item,tag) VALUES (?1,?2,?3)",
            rusqlite::params![kind, index, tag],
        )?;
        Ok(())
    }
    fn tag(&self, kind: &str, index: u64) -> Result<Vec<u8>> {
        let index = i64::try_from(index)
            .map_err(|_| GardenError::Recovery("Replay scratch index is too large.".into()))?;
        Ok(self.auth.query_row(
            "SELECT tag FROM tags WHERE kind=?1 AND item=?2",
            rusqlite::params![kind, index],
            |row| row.get(0),
        )?)
    }
    fn verify_tag(&self, kind: &str, index: u64, bytes: &[u8]) -> Result<()> {
        let mut mac = Mac::new(&self.auth_key, kind, index);
        mac.update(bytes);
        if self.tag(kind, index)? != mac.finish() {
            return Err(GardenError::Recovery(
                "Replay scratch changed after verification.".into(),
            ));
        }
        Ok(())
    }
    fn check(&self) -> Result<()> {
        if identity(&self.input.get_ref().metadata()?) != self.identity
            || identity(&migration_backup::open_nofollow(&self.root, &self.relative)?.metadata()?)
                != self.identity
        {
            return Err(GardenError::Recovery(
                "Publication manifest changed during replay preparation.".into(),
            ));
        }
        Ok(())
    }
    pub(super) fn verify(&self) -> Result<()> {
        self.check()
    }
    pub(super) fn verify_digest(
        &mut self,
        expected: Option<&migration_backup::replay_authority::Expected>,
    ) -> Result<()> {
        let digest = self.digest.take().ok_or_else(|| {
            GardenError::Recovery("Publication manifest digest was already consumed.".into())
        })?;
        if let Some(expected) = expected {
            digest.verify(expected)?;
        }
        Ok(())
    }
    fn prefix(&mut self, byte: u8, expected: &[u8], next: Phase) -> Result<()> {
        if !self.quoted && byte.is_ascii_whitespace() {
            return Ok(());
        }
        if expected.get(self.pos) != Some(&byte) {
            return Err(GardenError::Recovery(
                "Publication manifest framing is invalid.".into(),
            ));
        }
        if byte == b'"' {
            self.quoted = !self.quoted;
        }
        self.pos += 1;
        if self.pos == expected.len() {
            self.pos = 0;
            self.quoted = false;
            self.phase = next;
        }
        Ok(())
    }
    fn start(&mut self, byte: u8, kind: &str, index: u64) -> Result<()> {
        let path = self.work.join(kind).join(index.to_string());
        let mut output = ScratchOutput::open(&self.root, &path)?;
        output.push(byte);
        self.output = Some(output);
        let mut mac = Mac::new(&self.auth_key, kind, index);
        mac.update(&[byte]);
        self.record_mac = Some(mac);
        self.depth = if byte == b'{' { 1 } else { 0 };
        self.in_string = byte == b'"';
        self.escaped = false;
        Ok(())
    }
    fn finish(&mut self, kind: &str, index: u64) -> Result<()> {
        let output = self.output.take().unwrap();
        output.finish()?;
        let file = migration_backup::open_nofollow(
            &self.root,
            &self.work.join(kind).join(index.to_string()),
        )?;
        let tag = self
            .record_mac
            .take()
            .ok_or_else(|| {
                GardenError::Recovery("Replay scratch authentication state is missing.".into())
            })?
            .finish();
        if kind != "entries" {
            if file.metadata()?.len() > 131072 {
                return Err(GardenError::Recovery(
                    "Publication removal is too large.".into(),
                ));
            }
            let item: String = serde_json::from_reader(file)?;
            validate_relative_collection_path(Path::new(&item))?;
        }
        self.store_tag(kind, index, &tag)?;
        Ok(())
    }
    pub(super) fn step(&mut self) -> Result<bool> {
        self.check()?;
        if self.phase == Phase::Done {
            if self.decoded == self.entries {
                return Ok(true);
            }
            if self.decoder.is_none() {
                self.decoder = Some(super::replay_entry::Decoder::new(
                    &self.root,
                    &self.work,
                    self.decoded,
                    self.auth_key,
                    self.tag("entries", self.decoded)?,
                )?);
                return Ok(false);
            }
            if let Some(metadata) = self.decoder.as_mut().unwrap().step()? {
                let metadata_bytes = serde_json::to_vec(&metadata)?;
                let mut output = ScratchOutput::open(
                    &self.root,
                    &self.work.join("metadata").join(self.decoded.to_string()),
                )?;
                for byte in &metadata_bytes {
                    output.push(*byte);
                }
                output.finish()?;
                let mut mac = Mac::new(&self.auth_key, "metadata", self.decoded);
                mac.update(&metadata_bytes);
                self.store_tag("metadata", self.decoded, &mac.finish())?;
                self.decoded += 1;
                self.decoder = None;
            }
            return Ok(false);
        }
        for _ in 0..BUDGET {
            let mut b = [0];
            let count = self.input.read(&mut b)?;
            if count == 0 {
                if self.phase == Phase::Tail && self.pos == 1 {
                    self.phase = Phase::Done;
                    self.check()?;
                    return Ok(false);
                }
                return Err(GardenError::Recovery(
                    "Publication manifest is truncated.".into(),
                ));
            }
            let byte = b[0];
            self.digest.as_mut().unwrap().push(byte);
            match self.phase {
                Phase::Prefix => self.prefix(byte, b"{\"entries\":[", Phase::EntryFirst)?,
                Phase::EntryFirst | Phase::EntryRequired => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    if byte == b']' && self.phase == Phase::EntryFirst {
                        self.phase = Phase::AfterEntries;
                    } else if byte == b'{' {
                        self.start(byte, "entries", self.entries)?;
                        self.phase = Phase::Entry;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication entry is invalid.".into(),
                        ));
                    }
                }
                Phase::Entry => {
                    self.output.as_mut().unwrap().push(byte);
                    self.record_mac.as_mut().unwrap().update(&[byte]);
                    if self.in_string {
                        if self.escaped {
                            self.escaped = false
                        } else if byte == b'\\' {
                            self.escaped = true
                        } else if byte == b'"' {
                            self.in_string = false
                        }
                    } else if byte == b'"' {
                        self.in_string = true
                    } else if byte == b'{' || byte == b'[' {
                        self.depth += 1
                    } else if byte == b'}' || byte == b']' {
                        self.depth = self.depth.checked_sub(1).ok_or_else(|| {
                            GardenError::Recovery("Publication entry framing is invalid.".into())
                        })?;
                        if self.depth == 0 {
                            self.finish("entries", self.entries)?;
                            self.entries += 1;
                            self.phase = Phase::EntrySeparator;
                        }
                    }
                }
                Phase::EntrySeparator => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    if byte == b',' {
                        self.phase = Phase::EntryRequired;
                    } else if byte == b']' {
                        self.phase = Phase::AfterEntries;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication entry separator is invalid.".into(),
                        ));
                    }
                }
                Phase::AfterEntries => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    if byte == b'}' {
                        self.phase = Phase::Tail;
                        self.pos = 1;
                    } else if byte == b',' {
                        self.phase = Phase::RemovalPrefix;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication manifest fields are invalid.".into(),
                        ));
                    }
                }
                Phase::RemovalPrefix => {
                    self.prefix(byte, b"\"removals\":[", Phase::RemovalFirst)?
                }
                Phase::RemovalFirst | Phase::RemovalRequired => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    if byte == b']' && self.phase == Phase::RemovalFirst {
                        self.phase = Phase::Tail;
                    } else if byte == b'"' {
                        self.start(byte, "removals", self.removals)?;
                        self.phase = Phase::Removal;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication removal is invalid.".into(),
                        ));
                    }
                }
                Phase::Removal => {
                    self.output.as_mut().unwrap().push(byte);
                    self.record_mac.as_mut().unwrap().update(&[byte]);
                    if self.escaped {
                        self.escaped = false
                    } else if byte == b'\\' {
                        self.escaped = true
                    } else if byte == b'"' {
                        self.finish("removals", self.removals)?;
                        self.removals += 1;
                        self.phase = Phase::RemovalSeparator;
                    }
                }
                Phase::RemovalSeparator => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    if byte == b',' {
                        self.phase = Phase::RemovalRequired;
                    } else if byte == b']' {
                        self.phase = Phase::Tail;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication removal separator is invalid.".into(),
                        ));
                    }
                }
                Phase::Tail => {
                    if byte.is_ascii_whitespace() {
                        continue;
                    }
                    if byte == b'}' && self.pos == 0 {
                        self.pos = 1;
                    } else {
                        return Err(GardenError::Recovery(
                            "Publication manifest has trailing data.".into(),
                        ));
                    }
                }
                Phase::Done => return Ok(true),
            }
        }
        if let Some(output) = self.output.as_mut() {
            output.flush()?;
        }
        self.check()?;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn altered_framed_entry_and_metadata_are_rejected() {
        for alter_metadata in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = fs::canonicalize(temp.path()).unwrap();
            let transaction = root.join("transaction");
            fs::create_dir(&transaction).unwrap();
            let manifest = PublicationManifest {
                entries: vec![PublicationEntry {
                    destination: "notes/page".into(),
                    staged: "files/0000".into(),
                    expected: None,
                }],
                removals: vec![],
            };
            fs::write(
                transaction.join("manifest.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            let mut scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
            if alter_metadata {
                while !scan.step().unwrap() {}
                let path = root.join(&scan.work).join("metadata/0");
                let text = fs::read_to_string(&path)
                    .unwrap()
                    .replace("notes/page", "notes/evil");
                fs::write(path, text).unwrap();
                assert!(scan.entry(0).is_err());
            } else {
                while scan.phase != Phase::Done {
                    assert!(!scan.step().unwrap());
                }
                let path = root.join(&scan.work).join("entries/0");
                let text = fs::read_to_string(&path)
                    .unwrap()
                    .replace("notes/page", "notes/evil");
                fs::write(path, text).unwrap();
                let mut rejected = false;
                for _ in 0..100 {
                    match scan.step() {
                        Err(_) => {
                            rejected = true;
                            break;
                        }
                        Ok(true) => break,
                        Ok(false) => (),
                    }
                }
                assert!(rejected);
            }
        }
    }
    #[test]
    fn scratch_restart_child() {
        let Some(root) = std::env::var_os("KG_SCRATCH_RESTART_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let mut scan = Scanner::new(&root, &root.join("transaction"), "pre-work-test").unwrap();
        assert!(!scan.step().unwrap());
        std::process::exit(73);
    }
    #[test]
    fn repeated_process_exits_reuse_and_finish_the_same_scratch() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join("transaction");
        fs::create_dir(&transaction).unwrap();
        let manifest = PublicationManifest {
            entries: vec![PublicationEntry {
                destination: "notes/page".into(),
                staged: "files/0000".into(),
                expected: Some(vec![9; 250_000]),
            }],
            removals: vec![],
        };
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        for _ in 0..3 {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "application::publication::replay_manifest::tests::scratch_restart_child",
                ])
                .env("KG_SCRATCH_RESTART_ROOT", &root)
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(73));
        }
        let generation = root.join(".recovery/migration/replay-scratch/pre-work-test");
        assert_eq!(fs::read_dir(&generation).unwrap().count(), 1);
        let mut scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        while !scan.step().unwrap() {}
        assert_eq!(scan.entry(0).unwrap().base_length, 250_000);
        assert_eq!(scan.base(0, 250_000).unwrap(), vec![9; 250_000]);
        while !scan.cleanup_step().unwrap() {}
        assert_eq!(fs::read_dir(generation).unwrap().count(), 0);
    }
    #[test]
    fn changed_partial_scratch_is_preserved_and_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join("transaction");
        fs::create_dir(&transaction).unwrap();
        let manifest = PublicationManifest {
            entries: vec![PublicationEntry {
                destination: "notes/page".into(),
                staged: "files/0000".into(),
                expected: Some(vec![9; 250_000]),
            }],
            removals: vec![],
        };
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::replay_manifest::tests::scratch_restart_child",
            ])
            .env("KG_SCRATCH_RESTART_ROOT", &root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73));
        let generation = root.join(".recovery/migration/replay-scratch/pre-work-test");
        let scratch = fs::read_dir(generation)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("entries/0");
        let mut owner = OpenOptions::new().append(true).open(&scratch).unwrap();
        owner.write_all(b"external scratch bytes").unwrap();
        owner.sync_all().unwrap();
        drop(owner);
        let expected = fs::read(&scratch).unwrap();
        let mut scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        let mut rejected = false;
        for _ in 0..100 {
            match scan.step() {
                Err(_) => {
                    rejected = true;
                    break;
                }
                Ok(true) => break,
                Ok(false) => (),
            }
        }
        assert!(rejected);
        assert_eq!(fs::read(scratch).unwrap(), expected);
    }
    #[test]
    fn interrupted_origin_record_resumes_from_its_matching_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join("transaction");
        fs::create_dir(&transaction).unwrap();
        fs::write(transaction.join("manifest.json"), br#"{"entries":[]}"#).unwrap();
        let scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        let origin = root.join(&scan.work).join("origin.json");
        let expected = fs::read(&origin).unwrap();
        drop(scan);
        OpenOptions::new()
            .write(true)
            .open(&origin)
            .unwrap()
            .set_len(5)
            .unwrap();
        let _resumed = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        assert_eq!(fs::read(origin).unwrap(), expected);
    }
    #[test]
    fn linked_scratch_is_rejected_without_changing_the_other_link() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join("transaction");
        fs::create_dir(&transaction).unwrap();
        let manifest = PublicationManifest {
            entries: vec![PublicationEntry {
                destination: "notes/page".into(),
                staged: "files/0000".into(),
                expected: Some(vec![9; 250_000]),
            }],
            removals: vec![],
        };
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::publication::replay_manifest::tests::scratch_restart_child",
            ])
            .env("KG_SCRATCH_RESTART_ROOT", &root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73));
        let generation = root.join(".recovery/migration/replay-scratch/pre-work-test");
        let scratch = fs::read_dir(generation)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("entries/0");
        fs::remove_file(&scratch).unwrap();
        let outside = root.join("owner-data");
        fs::write(&outside, b"owner bytes").unwrap();
        fs::hard_link(&outside, &scratch).unwrap();
        let mut scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        assert!(scan.step().is_err());
        assert_eq!(fs::read(outside).unwrap(), b"owner bytes");
    }
    #[test]
    fn malformed_manifest_retains_framed_scratch_without_recursive_drop() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join("transaction");
        fs::create_dir(&transaction).unwrap();
        fs::write(transaction.join("manifest.json"), br#"{"entries":[{"destination":"notes/a","staged":"files/0000","expected":null},broken]}"#).unwrap();
        let mut scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        let work = scan.work.clone();
        assert!(scan.step().is_err());
        drop(scan);
        assert!(root.join(&work).join("origin.json").is_file());
        assert!(root.join(&work).join("entries/0").is_file());
        for _ in 0..3 {
            let mut restarted = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
            assert_eq!(restarted.work, work);
            assert!(restarted.step().is_err());
        }
        assert_eq!(
            fs::read_dir(root.join(".recovery/migration/replay-scratch/pre-work-test"))
                .unwrap()
                .count(),
            1
        );
    }
    #[test]
    fn scans_large_manifest_in_bounded_reads_and_rejects_trailing_comma() {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap();
        let transaction = root.join("transaction");
        fs::create_dir(&transaction).unwrap();
        let manifest = PublicationManifest {
            entries: vec![PublicationEntry {
                destination: "notes/page".into(),
                staged: "files/0000".into(),
                expected: Some(vec![9; 250_000]),
            }],
            removals: vec!["notes/old".into()],
        };
        fs::write(
            transaction.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let mut scan = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        let mut turns = 0;
        while !scan.step().unwrap() {
            turns += 1;
            assert!(turns < 100);
        }
        assert!(turns > 1);
        assert_eq!(scan.entry_count(), 1);
        assert_eq!(scan.removal_count(), 1);
        assert_eq!(scan.entry(0).unwrap().base_length, 250_000);
        assert_eq!(scan.base(0, 250_000).unwrap(), vec![9; 250_000]);
        assert_eq!(scan.removal(0).unwrap(), "notes/old");
        fs::write(transaction.join("manifest.json"), b"{\"entries\":[,]}").unwrap();
        let mut bad = Scanner::new(&root, &transaction, "pre-work-test").unwrap();
        assert!(bad.step().is_err());
    }
}
