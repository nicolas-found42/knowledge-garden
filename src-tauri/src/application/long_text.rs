//! Resumable plain-text formation. Provider requests are small; final reconciliation
//! has a separate, explicit ceiling so an oversized source never silently completes.
use super::*;
use crate::{
    semantic::TextChunkJob,
    text_chunk_staging::{self, ChunkStore, StagedChunk},
    text_chunks,
};

pub const PROVIDER_CHUNK_BYTES: usize = 32 * 1024;
pub const MAX_RECONCILIATION_BYTES: u64 = 32 * 1024 * 1024;

impl Application {
    fn text_chunk_directory(
        &self,
        source_id: &str,
        version: &str,
        generation: &str,
    ) -> Result<PathBuf> {
        let key = hex::encode(Sha256::digest(
            format!("{source_id}\0{version}\0{generation}").as_bytes(),
        ));
        let directory = self.root.join(".recovery/text-chunks").join(key);
        validate_real_collection_ancestors(&self.root, &directory.join("entry"))?;
        Ok(directory)
    }

    pub(super) fn claim_text_chunk(
        &mut self,
        page: &SourcePage,
        version: &SourceVersion,
        candidate: &Path,
    ) -> Result<(String, TextChunkJob)> {
        if version.bytes > MAX_RECONCILIATION_BYTES {
            return Err(GardenError::Invalid("Text exceeds the 32 MiB reconciliation limit. The complete original is retained; semantic coverage is incomplete.".into()));
        }
        let generation = self.source_generation(&page.info.source_id)?;
        let directory = self.text_chunk_directory(
            &page.info.source_id,
            &version.source_version_id,
            &generation,
        )?;
        let store = ChunkStore::open(
            &directory,
            &version.source_version_id,
            &generation,
            version.bytes,
        )?;
        let snapshot =
            match self.verified_asset_snapshot(candidate, &version.sha256, &version.format) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    self.record_original_integrity_failure(
                        &page.info.source_id,
                        &version.source_version_id,
                        &error,
                    )?;
                    return Err(GardenError::Recovery(error.to_string()));
                }
            };
        let offset = store.offset();
        let Some(chunk) =
            text_chunks::read_window(snapshot.path(), offset, PROVIDER_CHUNK_BYTES, 1024)?
        else {
            return Ok((
                String::new(),
                TextChunkJob {
                    start: offset,
                    context_start: offset,
                    end: offset,
                    total: version.bytes,
                },
            ));
        };
        Ok((
            chunk.text,
            TextChunkJob {
                start: offset,
                context_start: chunk.start,
                end: chunk.end,
                total: version.bytes,
            },
        ))
    }

    // Called only after the active version, generation and lease were checked.
    // A committed chunk is not knowledge: publication waits for the complete chain.
    pub(super) fn stage_text_chunk(
        &mut self,
        page: &mut SourcePage,
        job: &mut SemanticJob,
        mut draft: KnowledgeDraft,
    ) -> Result<Option<KnowledgeDraft>> {
        let range = job
            .text_chunk
            .as_ref()
            .ok_or_else(|| GardenError::Invalid("Missing text range.".into()))?;
        let version = page
            .info
            .versions_seen
            .iter()
            .find(|v| v.source_version_id == job.source_version_id)
            .cloned()
            .ok_or_else(|| GardenError::Invalid("Missing chunk source version.".into()))?;
        if range.total != version.bytes || range.total > MAX_RECONCILIATION_BYTES {
            return Err(GardenError::Invalid(
                "Chunk total does not match its bounded retained source.".into(),
            ));
        }
        let version_path = self.version_original_path(&job.source_id, &version)?;
        let path = if version_path.exists() {
            version_path
        } else if page.info.current_version_id.is_none() {
            self.source_dir(&job.source_id)?.join(&version.asset)
        } else {
            version_path
        };
        let snapshot = match self.verified_asset_snapshot(&path, &version.sha256, &version.format) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.record_original_integrity_failure(
                    &job.source_id,
                    &job.source_version_id,
                    &error,
                )?;
                return Err(GardenError::Recovery(error.to_string()));
            }
        };
        let directory =
            self.text_chunk_directory(&job.source_id, &job.source_version_id, &job.generation)?;
        let mut store = ChunkStore::open(
            &directory,
            &job.source_version_id,
            &job.generation,
            range.total,
        )?;
        if range.start < range.total {
            let exact =
                text_chunks::read_window(snapshot.path(), range.start, PROVIDER_CHUNK_BYTES, 1024)?
                    .ok_or_else(|| GardenError::Invalid("Claimed chunk is absent.".into()))?;
            if exact.start != range.context_start
                || exact.text != job.source_text
                || exact.end != range.end
                || store.offset() != range.start
            {
                return Err(GardenError::Invalid(
                    "Chunk does not match its retained bytes or checkpoint.".into(),
                ));
            }
            text_chunk_staging::validate_and_rebase(&mut draft, &exact.text, range.context_start)?;
            store.commit(&StagedChunk {
                source_version_id: job.source_version_id.clone(),
                generation: job.generation.clone(),
                start: range.start,
                end: range.end,
                draft,
            })?;
        } else if range.start != range.total
            || range.end != range.total
            || store.offset() != range.total
        {
            return Err(GardenError::Invalid(
                "Finalization requires a complete contiguous chunk chain.".into(),
            ));
        }
        if store.offset() != range.total {
            if page.info.current_version_id.is_some() {
                page.info.update_status = Some("pending".into());
            } else {
                page.info.semantic_state = "pending".into();
            }
            page.info.semantic_claim = None;
            page.info.semantic_retry_at = None;
            if let Some(version) = page
                .info
                .versions_seen
                .iter_mut()
                .find(|v| v.source_version_id == job.source_version_id)
            {
                version.state = "pending".into();
                version.processing_interruptions = 0;
                version.semantic_integrity_failures = 0;
                version.retryable_provider_failures = 0;
            }
            self.write_source_page(page)?;
            self.index_page(&page.info)?;
            return Ok(None);
        }
        let complete = text_chunk_staging::aggregate(
            &directory,
            &job.source_version_id,
            &job.generation,
            range.total,
        )?;
        // This is a bounded final reconciliation buffer, never a provider input.
        let mut bytes = Vec::new();
        File::open(snapshot.path())?
            .take(MAX_RECONCILIATION_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != range.total {
            return Err(GardenError::Invalid("Final text length changed.".into()));
        }
        job.source_text = String::from_utf8(bytes).map_err(|_| {
            GardenError::Invalid("The complete retained source is not UTF-8.".into())
        })?;
        job.text_chunk = None;
        page.info.line_count = job.source_text.lines().count();
        if page.info.current_version_id.is_none() {
            page.info.extraction = ExtractionState::TextPreserved;
            page.info.extraction_detail = "The complete retained UTF-8 text was processed in resumable chunks; the readable source preview is limited to 2 MiB.".into();
            page.body = source_body(&page.info, &job.source_text, None);
        }
        Ok(Some(complete))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoProvider;
    impl SemanticProvider for NoProvider {
        fn form_knowledge(&self, _: &str) -> std::result::Result<KnowledgeDraft, ProviderError> {
            panic!("A complete durable chain must finalize without another provider request")
        }
    }
    #[test]
    fn restart_after_last_committed_window_finalizes_without_reinference() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("collection");
        let input = temp.path().join("text.txt");
        fs::write(&input, "x".repeat(PROVIDER_CHUNK_BYTES + 10)).unwrap();
        let mut app =
            Application::open_with_semantic_provider(&root, Arc::new(NoProvider)).unwrap();
        let source = app
            .import_source(&input, AcquisitionMethod::Picker)
            .unwrap();
        let job = app.claim_due_semantic_jobs(1).unwrap().remove(0);
        let total = source.info.bytes;
        let directory = app
            .text_chunk_directory(&job.source_id, &job.source_version_id, &job.generation)
            .unwrap();
        let mut store =
            ChunkStore::open(&directory, &job.source_version_id, &job.generation, total).unwrap();
        while store.offset() < total {
            let start = store.offset();
            let chunk = text_chunks::read_window(&input, start, PROVIDER_CHUNK_BYTES, 1024)
                .unwrap()
                .unwrap();
            store
                .commit(&StagedChunk {
                    source_version_id: job.source_version_id.clone(),
                    generation: job.generation.clone(),
                    start,
                    end: chunk.end,
                    draft: KnowledgeDraft::default(),
                })
                .unwrap();
        }
        drop(store);
        Application::record_controlled_shutdown(&root).unwrap();
        drop(app);
        let mut app =
            Application::open_with_semantic_provider(&root, Arc::new(NoProvider)).unwrap();
        app.resume_due_semantic_jobs_at(u128::MAX).unwrap();
        assert_eq!(
            app.open_source(&source.info.source_id)
                .unwrap()
                .info
                .semantic_state,
            "complete"
        );
    }
}
