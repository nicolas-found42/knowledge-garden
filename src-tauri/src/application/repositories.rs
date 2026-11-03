use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRegistration {
    pub repository_id: String,
    pub path: PathBuf,
    pub state: String,
    #[serde(default)]
    pub legacy: bool,
}

impl Application {
    pub fn recover_registration_removals(&mut self) -> Result<()> {
        self.recover_unregistering_watches()?;
        self.recover_unregistering_repositories()
    }

    pub(super) fn register_repository(&mut self, id: &str, path: &Path) -> Result<()> {
        self.recover_unregistering_repositories()?;
        let record = RepositoryRegistration {
            repository_id: id.to_owned(),
            path: path.to_owned(),
            state: "active".into(),
            legacy: false,
        };
        write_atomic(
            &self.repository_registration_path(id)?,
            &serde_json::to_vec(&record)?,
        )
    }

    fn repository_registration_path(&self, id: &str) -> Result<PathBuf> {
        let Some(digest) = id.strip_prefix("repository-") else {
            return Err(GardenError::Invalid("Invalid repository identity.".into()));
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(GardenError::Invalid("Invalid repository identity.".into()));
        }
        let path = self.root.join("repositories").join(format!("{id}.json"));
        validate_real_collection_ancestors(&self.root, &path)?;
        Ok(path)
    }

    fn stored_repositories(&self) -> Result<Vec<RepositoryRegistration>> {
        let root = self.root.join("repositories");
        validate_real_collection_ancestors(&self.root, &root.join("entry"))?;
        let mut records = Vec::new();
        if root.exists() {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    return Err(GardenError::Invalid(
                        "Repository registration is not a file.".into(),
                    ));
                }
                let record: RepositoryRegistration =
                    serde_json::from_slice(&fs::read(entry.path())?)?;
                if self.repository_registration_path(&record.repository_id)? != entry.path() {
                    return Err(GardenError::Invalid(
                        "Repository registration identity mismatch.".into(),
                    ));
                }
                records.push(record);
            }
        }
        Ok(records)
    }

    pub fn list_repositories(&self) -> Result<Vec<RepositoryRegistration>> {
        let mut records = self.stored_repositories()?;
        let mut legacy = HashMap::<String, RepositoryRegistration>::new();
        for entry in fs::read_dir(self.root.join("sources"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(source) = self.readable_source_entry(&entry) else {
                return Err(GardenError::Invalid("A source record is unreadable; repository ownership cannot be established safely.".into()));
            };
            let acquisitions = source
                .info
                .acquisitions
                .iter()
                .filter(|acquisition| {
                    acquisition.method == AcquisitionMethod::Repository
                        && acquisition.registration_id.is_none()
                })
                .collect::<Vec<_>>();
            if acquisitions.is_empty() {
                continue;
            }
            if let Some(context) = &source.info.repository {
                let (path, explicit) = legacy_repository_context(
                    context,
                    &acquisitions,
                    &source.info.source_id,
                    &source.info.format,
                );
                let candidate =
                    legacy
                        .entry(context.repository_id.clone())
                        .or_insert(RepositoryRegistration {
                            repository_id: context.repository_id.clone(),
                            path,
                            state: "legacy_explicit".into(),
                            legacy: true,
                        });
                if !explicit {
                    candidate.state = "legacy_ambiguous".into();
                }
            } else {
                for acquisition in acquisitions {
                    let id = format!(
                        "legacy-unresolved-{}",
                        hex::encode(Sha256::digest(acquisition.path.as_bytes()))
                    );
                    legacy.entry(id.clone()).or_insert(RepositoryRegistration {
                        repository_id: id,
                        path: PathBuf::from(&acquisition.path),
                        state: "legacy_ambiguous".into(),
                        legacy: true,
                    });
                }
            }
        }
        for candidate in legacy.into_values() {
            if let Some(registered) = records
                .iter_mut()
                .find(|record| record.repository_id == candidate.repository_id)
            {
                if registered.state == "active" {
                    registered.state = "legacy_ambiguous".into();
                }
            } else {
                records.push(candidate);
            }
        }
        records.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(records)
    }

    pub fn unregister_repository(&mut self, id: &str) -> Result<()> {
        self.recover_source_removals()?;
        self.recover_registration_removals()?;
        let path = self.repository_registration_path(id)?;
        if self
            .list_repositories()?
            .iter()
            .any(|record| record.repository_id == id && record.state == "legacy_ambiguous")
        {
            return Err(GardenError::Invalid(
                "Repository ownership is ambiguous; its sources are retained.".into(),
            ));
        }
        let mut record: RepositoryRegistration = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            self.list_repositories()?
                .into_iter()
                .find(|record| record.repository_id == id && record.state == "legacy_explicit")
                .ok_or_else(|| {
                    GardenError::Invalid(
                        "Repository ownership is ambiguous or unknown; its sources are retained."
                            .into(),
                    )
                })?
        };
        if record.state == "legacy_ambiguous" {
            return Err(GardenError::Invalid(
                "Repository ownership is ambiguous; its sources are retained.".into(),
            ));
        }
        if record.repository_id != id {
            return Err(GardenError::Invalid(
                "Repository registration identity mismatch.".into(),
            ));
        }
        record.state = "unregistering".into();
        write_atomic(&path, &serde_json::to_vec(&record)?)?;
        self.finish_unregister_repository(&record)
    }

    pub(super) fn recover_unregistering_repositories(&mut self) -> Result<()> {
        for record in self.stored_repositories()? {
            if record.state == "unregistering" {
                self.finish_unregister_repository(&record)?;
            }
        }
        Ok(())
    }

    fn finish_unregister_repository(&mut self, record: &RepositoryRegistration) -> Result<()> {
        let mut sources = Vec::new();
        for entry in fs::read_dir(self.root.join("sources"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(source) = self.readable_source_entry(&entry) else {
                return Err(GardenError::Invalid("A source record is unreadable; repository ownership cannot be established safely.".into()));
            };
            for acquisition in &source.info.acquisitions {
                if acquisition.method != AcquisitionMethod::Repository {
                    continue;
                }
                if record.legacy {
                    if acquisition.registration_id.is_some() {
                        continue;
                    }
                    let Some(context) = source
                        .info
                        .repository
                        .as_ref()
                        .filter(|context| context.repository_id == record.repository_id)
                    else {
                        continue;
                    };
                    let legacy_acquisitions = source
                        .info
                        .acquisitions
                        .iter()
                        .filter(|item| {
                            item.method == AcquisitionMethod::Repository
                                && item.registration_id.is_none()
                        })
                        .collect::<Vec<_>>();
                    if !legacy_repository_context(
                        context,
                        &legacy_acquisitions,
                        &source.info.source_id,
                        &source.info.format,
                    )
                    .1
                    {
                        return Err(GardenError::Invalid("Legacy repository ownership became ambiguous; retained sources are unchanged.".into()));
                    }
                    sources.push((
                        source.info.source_id.clone(),
                        acquisition_origin(acquisition)?,
                    ));
                } else if acquisition.registration_id.as_deref() == Some(&record.repository_id) {
                    sources.push((
                        source.info.source_id.clone(),
                        acquisition_origin(acquisition)?,
                    ));
                }
            }
        }
        for (source_id, origin) in sources {
            if record.legacy {
                self.withdraw_legacy_repository_acquisition(&source_id, &origin)?;
            } else {
                self.withdraw_repository_acquisition(&source_id, &origin, &record.repository_id)?;
            }
        }
        commit_publication_with_removals(
            &self.root,
            Vec::new(),
            vec![self.repository_registration_path(&record.repository_id)?],
            &HashMap::new(),
            None,
        )?;
        Ok(())
    }
}

fn legacy_repository_context(
    context: &RepositorySourceContext,
    acquisitions: &[&Acquisition],
    source_id: &str,
    format: &str,
) -> (PathBuf, bool) {
    let raw = PathBuf::from(&context.repository_path);
    let decoded = decoded_repository_path(&context.repository_path).unwrap_or_else(|_| raw.clone());
    let mut roots = vec![raw.clone(), decoded];
    roots.sort();
    roots.dedup();
    let matching = roots
        .into_iter()
        .filter(|path| {
            format!(
                "repository-{}",
                hex::encode(Sha256::digest(path.as_os_str().as_encoded_bytes()))
            ) == context.repository_id
        })
        .collect::<Vec<_>>();
    let Some(root) = (matching.len() == 1).then(|| matching[0].clone()) else {
        return (raw, false);
    };
    let relative = decoded_repository_path(&context.relative_path).unwrap_or_default();
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return (root, false);
    }
    let expected = normalized_origin(&root.join(relative));
    let scope = format!(
        "repository-source:{format}:repository-file:{}:{}",
        context.repository_id, context.relative_path
    );
    let expected_source = format!("source-{}", hex::encode(Sha256::digest(scope.as_bytes())));
    let explicit = expected_source == source_id
        && acquisitions.len() == 1
        && acquisition_origin(acquisitions[0]).is_ok_and(|origin| origin == expected);
    (root, explicit)
}
