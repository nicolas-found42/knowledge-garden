//! Publish and revalidate one deterministic trusted-page record during cutover.
use super::*;

#[derive(Serialize, Deserialize)]
struct Receipt {
    path: PathBuf,
    sha256: String,
}

fn receipt_path(relative: &Path) -> Result<PathBuf> {
    let name = hex::encode(Sha256::digest(serde_json::to_vec(relative)?));
    Ok(
        PathBuf::from(".recovery/migration/retirement/trusted-receipts")
            .join(format!("{name}.json")),
    )
}
pub(super) fn existing_blob(root: &Path, bytes: &[u8]) -> Result<Option<(PathBuf, String)>> {
    let path =
        PathBuf::from(".recovery/trusted-pages/blobs").join(hex::encode(Sha256::digest(bytes)));
    match fs::symlink_metadata(root.join(&path)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
        Ok(m) if m.is_file() => {
            let mut actual = Vec::new();
            bounded_copy::source_file(root, &path)?
                .take((bytes.len() + 1) as u64)
                .read_to_end(&mut actual)?;
            if actual != bytes {
                return Err(GardenError::Recovery(
                    "Historical trusted blob differs from snapshot bytes.".into(),
                ));
            }
            Ok(Some((path, manifest_cursor::digest_bytes(bytes))))
        }
        Ok(_) => Err(GardenError::Recovery(
            "Historical trusted blob is not a regular file.".into(),
        )),
    }
}

/// Reconstruct only the expected trusted files already present. An absent
/// receipt permits an interrupted partial publication, never mismatched bytes.
pub(super) fn existing_expected(
    root: &Path,
    work: &Path,
    relative: &Path,
    bytes: &[u8],
) -> Result<Vec<(PathBuf, String)>> {
    validate_relative_collection_path(relative)?;
    if bytes.len() as u64 > MAX_PAGE_BYTES {
        return Err(GardenError::Recovery(
            "Trusted retirement page is too large.".into(),
        ));
    }
    let receipt = receipt_path(relative)?;
    let has_receipt = match fs::symlink_metadata(work.join(&receipt)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
        Ok(m) if m.is_file() => {
            let mut raw = Vec::new();
            bounded_copy::source_file(work, &receipt)?
                .take(16385)
                .read_to_end(&mut raw)?;
            if raw.len() > 16384 {
                return Err(GardenError::Recovery(
                    "Trusted retirement receipt is too large.".into(),
                ));
            }
            let value: Receipt = serde_json::from_slice(&raw)?;
            if value.path != relative || value.sha256 != hex::encode(Sha256::digest(bytes)) {
                return Err(GardenError::Recovery(
                    "Trusted retirement receipt differs from authenticated bytes.".into(),
                ));
            }
            true
        }
        Ok(_) => {
            return Err(GardenError::Recovery(
                "Trusted retirement receipt is not a regular file.".into(),
            ))
        }
    };
    let mut result = Vec::new();
    for (path, expected) in trusted_pages::publication_files(root, relative, bytes)? {
        let path = path
            .strip_prefix(root)
            .map_err(|_| {
                GardenError::Recovery("Trusted retirement record escaped the collection.".into())
            })?
            .to_path_buf();
        if !path.starts_with(".recovery/trusted-pages") {
            return Err(GardenError::Recovery(
                "Trusted retirement record has an invalid path.".into(),
            ));
        }
        match fs::symlink_metadata(root.join(&path)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && !has_receipt => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(GardenError::Recovery(
                    "A receipted trusted record is missing.".into(),
                ))
            }
            Err(e) => return Err(e.into()),
            Ok(m) if m.is_file() => {
                let mut actual = Vec::new();
                bounded_copy::source_file(root, &path)?
                    .take((expected.len() + 1) as u64)
                    .read_to_end(&mut actual)?;
                if actual != expected {
                    return Err(GardenError::Recovery(
                        "Trusted retirement record differs from authenticated output.".into(),
                    ));
                }
                result.push((path, manifest_cursor::digest_bytes(&expected)));
            }
            Ok(_) => {
                return Err(GardenError::Recovery(
                    "Trusted retirement record is not a regular file.".into(),
                ))
            }
        }
    }
    if has_receipt && trusted_pages::read(root, relative)? != bytes {
        return Err(GardenError::Recovery(
            "Trusted retirement record failed verification.".into(),
        ));
    }
    Ok(result)
}

/// The caller supplies bytes already authenticated against the snapshot or a
/// rederived retirement output, and holds the collection lock.
pub(super) fn record_one(root: &Path, work: &Path, relative: &Path, bytes: &[u8]) -> Result<()> {
    validate_relative_collection_path(relative)?;
    if bytes.len() as u64 > MAX_PAGE_BYTES {
        return Err(GardenError::Recovery(
            "Trusted retirement page is too large.".into(),
        ));
    }
    let records = trusted_pages::publication_files(root, relative, bytes)?;
    for (path, expected) in records {
        let path = path.strip_prefix(root).map_err(|_| {
            GardenError::Recovery("Trusted retirement record escaped the collection.".into())
        })?;
        if !path.starts_with(".recovery/trusted-pages") {
            return Err(GardenError::Recovery(
                "Trusted retirement record has an invalid path.".into(),
            ));
        }
        authored_stage::publish_once(root, path, &expected)?;
    }
    if trusted_pages::read(root, relative)? != bytes {
        return Err(GardenError::Recovery(
            "Trusted retirement record failed verification.".into(),
        ));
    }
    #[cfg(test)]
    if std::env::var_os("KG_TRUSTED_RETIRE_STOP_AFTER_RECORD").is_some() {
        std::process::exit(95);
    }
    let receipt = Receipt {
        path: relative.to_owned(),
        sha256: hex::encode(Sha256::digest(bytes)),
    };
    authored_stage::publish_once(
        work,
        &receipt_path(relative)?,
        &serde_json::to_vec(&receipt)?,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child() {
        let Some(base) = std::env::var_os("KG_TRUSTED_RETIRE_ROOT") else {
            return;
        };
        let base = PathBuf::from(base);
        let root = base.join("collection");
        let work = base.join("work");
        record_one(&root, &work, Path::new("pages/page.md"), b"verified page").unwrap();
        panic!("trusted-record hook did not run");
    }
    #[test]
    fn published_trusted_record_is_rechecked_after_exit_and_collision_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        fs::create_dir_all(base.join("collection/pages")).unwrap();
        fs::create_dir(base.join("work")).unwrap();
        fs::write(base.join("collection/pages/page.md"), b"verified page").unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "application::migration_backup::trusted_retirement::tests::child",
            ])
            .env("KG_TRUSTED_RETIRE_ROOT", base)
            .env("KG_TRUSTED_RETIRE_STOP_AFTER_RECORD", "1")
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(95));
        record_one(
            &base.join("collection"),
            &base.join("work"),
            Path::new("pages/page.md"),
            b"verified page",
        )
        .unwrap();
        let digest = hex::encode(Sha256::digest(b"verified page"));
        let blob = base
            .join("collection/.recovery/trusted-pages/blobs")
            .join(digest);
        fs::write(&blob, b"external collision").unwrap();
        assert!(record_one(
            &base.join("collection"),
            &base.join("work"),
            Path::new("pages/page.md"),
            b"verified page"
        )
        .is_err());
        assert_eq!(fs::read(blob).unwrap(), b"external collision");
    }
}
