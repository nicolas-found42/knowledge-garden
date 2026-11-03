//! A restartable, byte-budgeted copy of one backup member. The caller owns the
//! collection lock; this worker never needs the application mutex.
use super::*;
use std::io::{Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;

#[derive(Serialize, Deserialize)]
struct CopyJournal {
    source: PathBuf,
    destination: PathBuf,
    device: u64,
    inode: u64,
    destination_device: u64,
    destination_inode: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    copied: u64,
    verified: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct CopyProgress {
    pub copied: u64,
    pub verified: u64,
    pub total: u64,
}

fn read_journal(root: &Path, journal: &Path) -> Result<CopyJournal> {
    let mut bytes = Vec::new();
    source_file(root, journal)?
        .take(16385)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(GardenError::Recovery(
            "Backup work journal is too large; retained files were not changed.".into(),
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) fn destination(root: &Path, journal: &Path) -> Result<PathBuf> {
    let state = read_journal(root, journal)?;
    validate_relative_collection_path(&state.destination)?;
    if !state.destination.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Backup destination is outside recovery.".into(),
        ));
    }
    Ok(state.destination)
}
pub(super) fn source(root: &Path, journal: &Path) -> Result<PathBuf> {
    let state = read_journal(root, journal)?;
    validate_relative_collection_path(&state.source)?;
    Ok(state.source)
}

pub(super) fn progress(root: &Path, journal: &Path) -> Result<CopyProgress> {
    validate_relative_collection_path(journal)?;
    if !journal.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Backup work must stay in migration recovery.".into(),
        ));
    }
    let path = root.join(journal);
    validate_real_collection_ancestors(root, &path)?;
    let state = read_journal(root, journal)?;
    if state.copied > state.length || state.verified > state.copied {
        return Err(GardenError::Recovery(
            "Backup work journal is invalid; retained files were not changed.".into(),
        ));
    }
    Ok(CopyProgress {
        copied: state.copied,
        verified: state.verified,
        total: state.length,
    })
}

fn member_parent(root: &Path, relative: &Path, create: bool) -> Result<File> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    validate_relative_collection_path(relative)?;
    let directory_flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = File::from(
        openat(CWD, root, directory_flags, Mode::empty()).map_err(std::io::Error::from)?,
    );
    for part in relative.parent().unwrap_or(Path::new("")).components() {
        if create {
            match mkdirat(
                &directory,
                part.as_os_str(),
                Mode::from_bits_truncate(0o700),
            ) {
                Ok(()) => directory.sync_all()?,
                Err(rustix::io::Errno::EXIST) => (),
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        directory = File::from(
            openat(&directory, part.as_os_str(), directory_flags, Mode::empty())
                .map_err(std::io::Error::from)?,
        );
    }
    Ok(directory)
}

fn open_member(root: &Path, relative: &Path, flags: rustix::fs::OFlags) -> Result<File> {
    use rustix::fs::{openat, Mode, OFlags};
    let directory = member_parent(root, relative, false)?;
    let name = relative
        .file_name()
        .ok_or_else(|| GardenError::Recovery("Backup source path is empty.".into()))?;
    let fd = openat(
        &directory,
        name,
        flags | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    Ok(fd.into())
}

fn create_member(root: &Path, relative: &Path) -> Result<(File, File)> {
    use rustix::fs::{openat, Mode, OFlags};
    let parent = member_parent(root, relative, true)?;
    let file = openat(
        &parent,
        relative.file_name().unwrap(),
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_bits_truncate(0o600),
    )
    .map_err(std::io::Error::from)?;
    Ok((file.into(), parent))
}

fn publish_journal(root: &Path, relative: &Path, bytes: &[u8]) -> Result<()> {
    use rustix::fs::{openat, renameat_with, unlinkat, AtFlags, Mode, OFlags, RenameFlags};
    let parent = member_parent(root, relative, true)?;
    let temporary = format!(".copy-journal-{}", uuid::Uuid::new_v4());
    let mut file = File::from(
        openat(
            &parent,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(std::io::Error::from)?,
    );
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        // Publish a fully written inode without replacing an existing journal.
        renameat_with(
            &parent,
            temporary.as_str(),
            &parent,
            relative.file_name().unwrap(),
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)?;
        parent.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        // A successful rename already removed this name; otherwise remove only
        // our unpublished temporary through the same held directory.
        let _ = unlinkat(&parent, temporary.as_str(), AtFlags::empty());
    }
    result
}

pub(super) fn source_file(root: &Path, relative: &Path) -> Result<File> {
    let file = open_member(
        root,
        relative,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK,
    )?;
    if !file.metadata()?.is_file() {
        return Err(GardenError::Recovery(
            "Recovery input is not a regular file.".into(),
        ));
    }
    Ok(file)
}

fn check_source(file: &File, state: &CopyJournal) -> Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.dev() != state.device
        || metadata.ino() != state.inode
        || metadata.len() != state.length
        || metadata.mtime() != state.modified_seconds
        || metadata.mtime_nsec() != state.modified_nanoseconds
    {
        return Err(GardenError::Recovery("A collection file changed during backup; its bytes and the partial copy remain preserved.".into()));
    }
    Ok(())
}

fn check_live_source(root: &Path, file: &File, state: &CopyJournal) -> Result<()> {
    check_source(file, state)?;
    let current = source_file(root, &state.source)?;
    check_source(&current, state)
}

/// Create a unique journal and destination. Both paths are relative to the
/// collection and must be in the migration recovery directory.
pub(super) fn begin(root: &Path, journal: &Path, source: &Path, destination: &Path) -> Result<()> {
    begin_inner(root, journal, source, destination, false).map(|_| ())
}

/// A small member can be durably copied and independently compared in one
/// bounded turn. Larger members leave no work behind and use normal cursors.
pub(super) fn begin_verified_small(
    root: &Path,
    journal: &Path,
    source: &Path,
    destination: &Path,
) -> Result<bool> {
    begin_inner(root, journal, source, destination, true)
}

fn begin_inner(
    root: &Path,
    journal: &Path,
    source: &Path,
    destination: &Path,
    verify_small: bool,
) -> Result<bool> {
    for path in [journal, destination] {
        validate_relative_collection_path(path)?;
        if !path.starts_with(".recovery/migration") {
            return Err(GardenError::Recovery(
                "Backup work must stay in migration recovery.".into(),
            ));
        }
        validate_real_collection_ancestors(root, &root.join(path))?;
    }
    let mut source_file = source_file(root, source)?;
    let metadata = source_file.metadata()?;
    if !metadata.is_file() {
        return Err(GardenError::Recovery(
            "Backup source is not a regular file.".into(),
        ));
    }
    if verify_small && metadata.len() > manifest_cursor::BLOCK as u64 {
        return Ok(false);
    }
    let (mut output, output_parent) = create_member(root, destination)?;
    let target_metadata = output.metadata()?;
    if verify_small {
        let mut bytes = vec![0; metadata.len() as usize];
        source_file.read_exact(&mut bytes)?;
        output.write_all(&bytes)?;
    }
    output.sync_all()?;
    output_parent.sync_all()?;
    let state = CopyJournal {
        source: source.to_owned(),
        destination: destination.to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
        destination_device: target_metadata.dev(),
        destination_inode: target_metadata.ino(),
        length: metadata.len(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        copied: if verify_small { metadata.len() } else { 0 },
        verified: if verify_small { metadata.len() } else { 0 },
    };
    if verify_small {
        source_file.seek(SeekFrom::Start(0))?;
        output.seek(SeekFrom::Start(0))?;
        let mut original = vec![0; metadata.len() as usize];
        let mut copied = vec![0; metadata.len() as usize];
        source_file.read_exact(&mut original)?;
        output.read_exact(&mut copied)?;
        check_live_source(root, &source_file, &state)?;
        if original != copied {
            return Err(GardenError::Recovery(
                "Backup verification found different bytes; both copies remain preserved.".into(),
            ));
        }
    }
    // Do not replace an existing work journal. A crash before this publication
    // leaves an inspectable destination, never a silently reused one.
    publish_journal(root, journal, &serde_json::to_vec(&state)?)?;
    Ok(true)
}

/// Copy or compare at most `budget` bytes. Returns true only when both passes
/// complete. Each durable cursor update follows a synced destination write.
pub(super) fn step(root: &Path, journal: &Path, budget: usize) -> Result<bool> {
    use rustix::fs::OFlags;
    validate_relative_collection_path(journal)?;
    if !journal.starts_with(".recovery/migration") {
        return Err(GardenError::Recovery(
            "Backup work must stay in migration recovery.".into(),
        ));
    }
    let journal_path = root.join(journal);
    validate_real_collection_ancestors(root, &journal_path)?;
    let mut state = read_journal(root, journal)?;
    if !state.destination.starts_with(".recovery/migration")
        || state.copied > state.length
        || state.verified > state.copied
    {
        return Err(GardenError::Recovery(
            "Backup work journal is invalid; retained files were not changed.".into(),
        ));
    }
    validate_relative_collection_path(&state.destination)?;
    let mut source = source_file(root, &state.source)?;
    check_live_source(root, &source, &state)?;
    let target = root.join(&state.destination);
    validate_real_collection_ancestors(root, &target)?;
    let mut output = open_member(root, &state.destination, OFlags::RDWR)?;
    let target_metadata = output.metadata()?;
    if !target_metadata.is_file()
        || target_metadata.dev() != state.destination_device
        || target_metadata.ino() != state.destination_inode
        || target_metadata.nlink() != 1
    {
        return Err(GardenError::Recovery(
            "A partial backup copy was replaced; retained files were not changed.".into(),
        ));
    }
    let target_len = target_metadata.len();
    if target_len < state.copied || target_len > state.length {
        return Err(GardenError::Recovery(
            "A partial backup copy changed; it remains preserved for inspection.".into(),
        ));
    }
    if state.copied == state.length && state.verified == state.length {
        return Ok(true);
    }
    // One sync per bounded batch, rather than one sync per small I/O buffer.
    let limit = budget.clamp(1, 4 * 1024 * 1024) as u64;
    if state.copied < state.length {
        let count = (state.length - state.copied).min(limit) as usize;
        source.seek(SeekFrom::Start(state.copied))?;
        let mut bytes = vec![0; count];
        source.read_exact(&mut bytes)?;
        output.seek(SeekFrom::Start(state.copied))?;
        let existing = (target_len - state.copied).min(count as u64) as usize;
        if existing != 0 {
            let mut tail = vec![0; existing];
            output.read_exact(&mut tail)?;
            if tail != bytes[..existing] {
                return Err(GardenError::Recovery(
                    "A partial backup copy conflicts with the source; both remain preserved."
                        .into(),
                ));
            }
        }
        output.write_all(&bytes[existing..])?;
        output.sync_all()?;
        check_live_source(root, &source, &state)?;
        state.copied += count as u64;
    } else if state.verified < state.length {
        let count = (state.length - state.verified).min(limit) as usize;
        source.seek(SeekFrom::Start(state.verified))?;
        output.seek(SeekFrom::Start(state.verified))?;
        let mut input = vec![0; count];
        let mut copied = vec![0; count];
        source.read_exact(&mut input)?;
        output.read_exact(&mut copied)?;
        check_live_source(root, &source, &state)?;
        if input != copied {
            return Err(GardenError::Recovery(
                "Backup verification found different bytes; both copies remain preserved.".into(),
            ));
        }
        state.verified += count as u64;
    }
    write_atomic(&journal_path, &serde_json::to_vec(&state)?)?;
    Ok(state.copied == state.length && state.verified == state.length)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swapped_destination_parent_cannot_redirect_copy_or_journal_creation() {
        use std::os::unix::fs::symlink;
        for journal in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("collection");
            let parent = root.join(".recovery/migration/work/temps");
            let outside = temp.path().join("outside");
            fs::create_dir_all(&parent).unwrap();
            fs::create_dir(&outside).unwrap();
            fs::write(root.join("owner"), b"retained original").unwrap();
            let relative = Path::new(".recovery/migration/work/temps/item");
            validate_real_collection_ancestors(&root, &root.join(relative)).unwrap();
            fs::rename(&parent, parent.with_file_name("retained-temps")).unwrap();
            symlink(&outside, &parent).unwrap();
            let result = if journal {
                publish_journal(&root, relative, b"retained journal")
            } else {
                create_member(&root, relative).map(|_| ())
            };
            assert!(result.is_err());
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
            assert_eq!(fs::read(root.join("owner")).unwrap(), b"retained original");
        }
    }

    #[test]
    fn scoped_journal_publication_never_clobbers_and_cleans_its_temporary() {
        let temp = tempfile::tempdir().unwrap();
        let relative = Path::new(".recovery/migration/work/journal.json");
        publish_journal(temp.path(), relative, b"prior journal").unwrap();
        assert!(publish_journal(temp.path(), relative, b"replacement").is_err());
        assert_eq!(
            fs::read(temp.path().join(relative)).unwrap(),
            b"prior journal"
        );
        assert_eq!(
            fs::read_dir(temp.path().join(relative.parent().unwrap()))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn verified_small_copy_preserves_bytes_and_rejects_replaced_source() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("owner"), b"retained original").unwrap();
        let journal = Path::new(".recovery/migration/work/copy.json");
        let destination = Path::new(".recovery/migration/work/original");
        assert!(begin_verified_small(root, journal, Path::new("owner"), destination).unwrap());
        assert!(step(root, journal, 1).unwrap());
        assert_eq!(
            fs::read(root.join(destination)).unwrap(),
            b"retained original"
        );
        fs::rename(root.join("owner"), root.join("previous-owner")).unwrap();
        fs::write(root.join("owner"), b"retained original").unwrap();
        assert!(step(root, journal, 1).is_err());
        assert_eq!(
            fs::read(root.join("previous-owner")).unwrap(),
            b"retained original"
        );
    }
    #[test]
    fn large_copy_keeps_the_resumable_path_without_starting_small_work() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::write(root.join("owner"), vec![42; manifest_cursor::BLOCK + 1]).unwrap();
        let journal = Path::new(".recovery/migration/work/copy.json");
        let destination = Path::new(".recovery/migration/work/original");
        assert!(!begin_verified_small(root, journal, Path::new("owner"), destination).unwrap());
        assert!(!root.join(destination).exists());
        assert!(!root.join(journal).exists());
        begin(root, journal, Path::new("owner"), destination).unwrap();
        assert!(!step(root, journal, 1024).unwrap());
    }
    #[test]
    fn fifo_input_is_rejected_without_waiting_for_a_writer() {
        use std::os::unix::ffi::OsStrExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("fifo");
        let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(source_file(temp.path(), Path::new("fifo")).is_err());
    }

    #[test]
    fn interrupted_copy_child() {
        let Some(root) = std::env::var_os("KG_BOUNDED_COPY_CHILD") else {
            return;
        };
        let root = PathBuf::from(root);
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        begin(&root, journal, Path::new("owner"), destination).unwrap();
        assert!(!step(&root, journal, 3).unwrap());
        std::process::exit(83);
    }

    #[test]
    fn abrupt_process_exit_resumes_copy_and_verification() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("owner"), b"retained owner bytes").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::bounded_copy::tests::interrupted_copy_child",
            ])
            .env("KG_BOUNDED_COPY_CHILD", root)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(83));
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        let mut turns = 0;
        while !step(root, journal, 3).unwrap() {
            turns += 1;
            assert!(turns < 30);
        }
        assert_eq!(
            fs::read(root.join(destination)).unwrap(),
            b"retained owner bytes"
        );
    }

    #[test]
    fn interrupted_copy_resumes_in_bounded_steps_and_verifies_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("owner"), b"a longer retained original").unwrap();
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        begin(root, journal, Path::new("owner"), destination).unwrap();
        assert!(!step(root, journal, 4).unwrap());
        assert_eq!(
            progress(root, journal).unwrap(),
            CopyProgress {
                copied: 4,
                verified: 0,
                total: 26
            }
        );
        assert_eq!(fs::read(root.join(destination)).unwrap(), b"a lo");
        let mut output = OpenOptions::new()
            .append(true)
            .open(root.join(destination))
            .unwrap();
        output.write_all(b"nger").unwrap();
        output.sync_all().unwrap();
        drop(output);
        assert!(!step(root, journal, 4).unwrap());
        let mut turns = 0;
        while !step(root, journal, 4).unwrap() {
            turns += 1;
            assert!(turns < 20);
        }
        assert_eq!(
            fs::read(root.join(destination)).unwrap(),
            b"a longer retained original"
        );
    }

    #[test]
    fn changed_source_or_partial_copy_remains_pending() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("owner"), b"original").unwrap();
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        begin(root, journal, Path::new("owner"), destination).unwrap();
        assert!(!step(root, journal, 3).unwrap());
        fs::write(root.join("owner"), b"changed!").unwrap();
        assert!(step(root, journal, 3).is_err());
        assert_eq!(fs::read(root.join("owner")).unwrap(), b"changed!");
        assert_eq!(fs::read(root.join(destination)).unwrap(), b"ori");
    }

    #[test]
    fn replaced_destination_ancestor_is_not_followed() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("owner"), b"original").unwrap();
        let journal = Path::new(".recovery/migration/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        begin(root, journal, Path::new("owner"), destination).unwrap();
        fs::rename(
            root.join(".recovery/migration/work"),
            root.join("displaced"),
        )
        .unwrap();
        symlink(
            root.join("displaced"),
            root.join(".recovery/migration/work"),
        )
        .unwrap();
        assert!(step(root, journal, 3).is_err());
        assert_eq!(fs::read(root.join("displaced/original")).unwrap(), b"");
    }

    #[test]
    fn replaced_partial_destination_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("owner"), b"original").unwrap();
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        begin(root, journal, Path::new("owner"), destination).unwrap();
        assert!(!step(root, journal, 3).unwrap());
        fs::rename(
            root.join(destination),
            root.join(".recovery/migration/work/displaced"),
        )
        .unwrap();
        fs::write(root.join(destination), b"another owner file").unwrap();
        assert!(step(root, journal, 3).is_err());
        assert_eq!(
            fs::read(root.join(destination)).unwrap(),
            b"another owner file"
        );
        assert_eq!(
            fs::read(root.join(".recovery/migration/work/displaced")).unwrap(),
            b"ori"
        );
    }

    #[test]
    fn replaced_source_with_same_bytes_stays_pending() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("owner"), b"original").unwrap();
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        begin(root, journal, Path::new("owner"), destination).unwrap();
        assert!(!step(root, journal, 3).unwrap());
        fs::rename(root.join("owner"), root.join("prior-owner")).unwrap();
        fs::write(root.join("owner"), b"original").unwrap();
        assert!(step(root, journal, 3).is_err());
        assert_eq!(fs::read(root.join(destination)).unwrap(), b"ori");
        assert_eq!(fs::read(root.join("prior-owner")).unwrap(), b"original");
    }

    #[test]
    fn symlinked_source_is_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("real-owner"), b"owner content").unwrap();
        symlink("real-owner", root.join("owner")).unwrap();
        let journal = Path::new(".recovery/migration/work/journal.json");
        let destination = Path::new(".recovery/migration/work/original");
        assert!(begin(root, journal, Path::new("owner"), destination).is_err());
        assert_eq!(fs::read(root.join("real-owner")).unwrap(), b"owner content");
        assert!(!root.join(destination).exists());
    }

    #[test]
    fn symlinked_source_ancestor_is_rejected() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("real")).unwrap();
        fs::write(root.join("real/owner"), b"owner content").unwrap();
        symlink("real", root.join("alias")).unwrap();
        assert!(begin(
            root,
            Path::new(".recovery/migration/work/journal.json"),
            Path::new("alias/owner"),
            Path::new(".recovery/migration/work/original")
        )
        .is_err());
        assert_eq!(fs::read(root.join("real/owner")).unwrap(), b"owner content");
    }
}

#[cfg(test)]
mod benchmark {
    use super::*;
    #[test]
    #[ignore]
    fn compare_durable_batches_on_256_mib_fixture() {
        let base = tempfile::tempdir().unwrap();
        for budget in [64 * 1024, 4 * 1024 * 1024] {
            let root = base.path().join(format!("size-{budget}"));
            fs::create_dir(&root).unwrap();
            let source = root.join("original");
            let mut file = File::create(&source).unwrap();
            let block = vec![0x5a; 1024 * 1024];
            for _ in 0..256 {
                file.write_all(&block).unwrap();
            }
            file.sync_all().unwrap();
            drop(file);
            let journal = Path::new(".recovery/migration/work/copy.json");
            let target = Path::new(".recovery/migration/work/file");
            begin(&root, journal, Path::new("original"), target).unwrap();
            let started = std::time::Instant::now();
            let mut turns = 0;
            loop {
                turns += 1;
                if step(&root, journal, budget).unwrap() {
                    break;
                }
            }
            let elapsed = started.elapsed();
            assert_eq!(
                fs::metadata(root.join(target)).unwrap().len(),
                256 * 1024 * 1024
            );
            assert_eq!(
                digest_file(&source).unwrap(),
                digest_file(&root.join(target)).unwrap()
            );
            eprintln!(
                "batch={budget} elapsed_ms={} turns={turns} bytes=268435456 verified=true",
                elapsed.as_millis()
            );
        }
    }
}
