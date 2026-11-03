use super::*;
use base64::Engine;
use std::io::Read;

pub struct VisualClaim {
    source_id: String,
    job: VisualJob,
    original: VerifiedVersionSnapshot,
    attempt: u32,
    audio_processor: Arc<dyn AudioProcessor>,
}

impl VisualClaim {
    pub fn run_with<F>(
        &self,
        infer: F,
    ) -> std::result::Result<crate::visual_inference::VisualInterpretation, String>
    where
        F: FnOnce(
            &[u8],
        )
            -> std::result::Result<crate::visual_inference::VisualInterpretation, String>,
    {
        let bytes = if let Some(at) = self.job.frame_ms {
            let workspace = tempfile::tempdir().map_err(|e| e.to_string())?;
            let frame = workspace.path().join("frame.jpg");
            self.audio_processor
                .extract_frame(self.original.path(), at, &frame)?;
            let mut bytes = Vec::new();
            fs::File::open(frame)
                .map_err(|e| e.to_string())?
                .take(8 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            bytes
        } else {
            let data = crate::photo::preview(self.original.path())?;
            let encoded = data
                .strip_prefix("data:image/jpeg;base64,")
                .ok_or("Photo preview is unavailable")?;
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .map_err(|e| e.to_string())?
        };
        infer(&bytes)
    }
    pub fn run(
        &self,
    ) -> std::result::Result<crate::visual_inference::VisualInterpretation, String> {
        self.run_with(|bytes| {
            #[cfg(unix)]
            {
                crate::visual_inference::ManagedVisualModel::system()
                    .interpret(bytes)
                    .map_err(|error| error.to_string())
            }
            #[cfg(not(unix))]
            {
                let _ = bytes;
                Err("Local visual runtime is unavailable on this platform.".into())
            }
        })
    }
}

impl Application {
    pub(super) fn discover_visual_jobs(&mut self) -> Result<()> {
        for entry in fs::read_dir(self.root.join("sources"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(mut page) = self.readable_source_entry(&entry) else {
                continue;
            };
            let Some(version_id) = page.info.pending_version_id.clone() else {
                continue;
            };
            let generation = self.source_generation(&page.info.source_id)?;
            let previous_count = page.info.visual_jobs.len();
            page.info
                .visual_jobs
                .retain(|job| job.generation == generation);
            let Some(version) = page
                .info
                .versions_seen
                .iter()
                .find(|v| v.source_version_id == version_id)
            else {
                continue;
            };
            let targets: Vec<Option<u64>> = if crate::photo::is_photo(&version.format) {
                vec![None]
            } else if is_video_format(&version.format) {
                candidate_audio_processing(&page.info, &version_id)
                    .filter(|audio| audio.state == "complete")
                    .map(|audio| {
                        audio
                            .frames
                            .iter()
                            .filter(|frame| frame.requested_ms % 30_000 == 0)
                            .map(|frame| Some(frame.requested_ms))
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let mut changed = previous_count != page.info.visual_jobs.len();
            for frame_ms in targets {
                if !page
                    .info
                    .visual_jobs
                    .iter()
                    .any(|job| job.source_version_id == version_id && job.frame_ms == frame_ms)
                {
                    page.info.visual_jobs.push(VisualJob {
                        source_version_id: version_id.clone(),
                        generation: generation.clone(),
                        frame_ms,
                        attempts: 0,
                        retry_at_ms: 0,
                        state: "pending".into(),
                        detail: "Local visual interpretation is queued.".into(),
                        result: None,
                    });
                    changed = true;
                }
            }
            if changed {
                self.write_source_page(&mut page)?;
                self.index_page(&page.info)?;
            }
        }
        Ok(())
    }

    pub fn resume_due_visual_jobs(&mut self) -> Result<()> {
        if let Some(job) = self.claim_due_visual_job_at(now_millis()? as u64)? {
            let result = job.run();
            self.finish_visual_job(job, result)?;
        }
        Ok(())
    }

    pub fn resume_due_visual_jobs_at_with<F>(&mut self, now: u64, infer: F) -> Result<()>
    where
        F: FnOnce(
            &[u8],
        )
            -> std::result::Result<crate::visual_inference::VisualInterpretation, String>,
    {
        if let Some(job) = self.claim_due_visual_job_at(now)? {
            let result = job.run_with(infer);
            self.finish_visual_job_at(job, result, now)?;
        }
        Ok(())
    }

    pub fn claim_due_visual_job(&mut self) -> Result<Option<VisualClaim>> {
        self.claim_due_visual_job_at(now_millis()? as u64)
    }

    /// Reserve an attempt durably; callers must run the returned job outside the collection lock.
    pub fn claim_due_visual_job_at(&mut self, now: u64) -> Result<Option<VisualClaim>> {
        self.require_integrity_recovered()?;
        self.recover_source_removals()?;
        self.discover_visual_jobs()?;
        let mut selected = None;
        for entry in fs::read_dir(self.root.join("sources"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(page) = self.readable_source_entry(&entry) else {
                continue;
            };
            let generation = self.source_generation(&page.info.source_id)?;
            if let Some(job) = page.info.visual_jobs.iter().find(|job| {
                (job.state == "pending" || job.state == "processing")
                    && job.generation == generation
                    && job.retry_at_ms <= now
                    && page.info.pending_version_id.as_deref()
                        == Some(job.source_version_id.as_str())
            }) {
                selected = Some((page.info.source_id.clone(), job.clone()));
                break;
            }
        }
        let Some((source_id, job)) = selected else {
            return Ok(None);
        };
        let mut page = self.open_source(&source_id)?;
        let Some(version) = page
            .info
            .versions_seen
            .iter()
            .find(|version| version.source_version_id == job.source_version_id)
            .cloned()
        else {
            return Ok(None);
        };
        let original =
            self.verified_version_snapshot(&source_id, &version.source_version_id, &version.asset)?;
        if job.attempts >= 5 {
            let record = page
                .info
                .visual_jobs
                .iter_mut()
                .find(|item| {
                    item.source_version_id == job.source_version_id && item.frame_ms == job.frame_ms
                })
                .unwrap();
            record.state = "failed".into();
            record.detail = "Local visual processing exhausted its interruption and retry budget; the original remains available.".into();
            note_failure(&mut page, &job);
            self.write_source_page(&mut page)?;
            self.index_page(&page.info)?;
            return Ok(None);
        }
        let attempt = {
            let record = page
                .info
                .visual_jobs
                .iter_mut()
                .find(|item| {
                    item.source_version_id == job.source_version_id && item.frame_ms == job.frame_ms
                })
                .unwrap();
            record.attempts = record.attempts.saturating_add(1);
            record.retry_at_ms = now.saturating_add(400_000);
            record.state = "processing".into();
            record.detail =
                "Local visual interpretation is running; an interruption will retry with backoff."
                    .into();
            record.attempts
        };
        self.write_source_page(&mut page)?;
        self.index_page(&page.info)?;
        Ok(Some(VisualClaim {
            source_id,
            job,
            original,
            attempt,
            audio_processor: self.audio_processor.clone(),
        }))
    }

    pub fn finish_visual_job(
        &mut self,
        claim: VisualClaim,
        result: std::result::Result<crate::visual_inference::VisualInterpretation, String>,
    ) -> Result<()> {
        self.finish_visual_job_at(claim, result, now_millis()? as u64)
    }

    pub fn finish_visual_job_at(
        &mut self,
        claim: VisualClaim,
        result: std::result::Result<crate::visual_inference::VisualInterpretation, String>,
        finished_at_ms: u64,
    ) -> Result<()> {
        let VisualClaim {
            source_id,
            job,
            original,
            attempt,
            ..
        } = claim;
        self.recover_source_removals()?;
        if self.source_generation(&source_id)? != job.generation {
            return Ok(());
        }
        self.verify_version_snapshot_current(&original)?;
        let mut page = self.open_source(&source_id)?;
        if page.info.pending_version_id.as_deref() != Some(job.source_version_id.as_str()) {
            return Ok(());
        }
        let Some(record) = page.info.visual_jobs.iter_mut().find(|item| {
            item.source_version_id == job.source_version_id
                && item.generation == job.generation
                && item.frame_ms == job.frame_ms
                && item.attempts == attempt
                && item.state == "processing"
        }) else {
            return Ok(());
        };
        match result {
            Ok(result) => {
                record.state = "complete".into();
                record.detail = "Unverified local visual interpretation is available.".into();
                record.result = Some(result.clone());
                if let Some(at) = job.frame_ms {
                    if let Some(audio) =
                        candidate_audio_processing_mut(&mut page.info, &job.source_version_id)
                    {
                        if let Some(frame) = audio
                            .frames
                            .iter_mut()
                            .find(|frame| frame.requested_ms == at)
                        {
                            frame.qwen_interpretation = Some(result);
                        }
                    }
                    page.body = source_body(&page.info, "", None);
                } else {
                    page.body.push_str(&format!("\n## Local visual interpretation\n\n{}\n\nUnverified model interpretation; may hallucinate. Model {} · observed revision {}.\n", escape_markdown(&result.description), escape_markdown(&result.model), escape_markdown(&result.observed_revision)));
                }
            }
            Err(error) => {
                record.state = "pending".into();
                record.retry_at_ms =
                    finished_at_ms.saturating_add((1_u64 << attempt.min(10)).min(3600) * 1000);
                record.detail = format!("Local visual interpretation failed: {error}");
                if attempt >= 5 {
                    record.state = "failed".into();
                    record.detail.push_str(
                        " The retry budget was exhausted; the original remains available.",
                    );
                }
            }
        }
        if page.info.visual_jobs.iter().any(|item| {
            item.source_version_id == job.source_version_id
                && item.frame_ms == job.frame_ms
                && item.state == "failed"
        }) {
            note_failure(&mut page, &job);
        }
        self.write_source_page(&mut page)?;
        self.index_page(&page.info)?;
        Ok(())
    }
}

fn note_failure(page: &mut SourcePage, job: &VisualJob) {
    let detail = "Local Qwen visual interpretation failed after bounded retries; the original remains available.";
    if let Some(at) = job.frame_ms {
        if let Some(audio) = candidate_audio_processing_mut(&mut page.info, &job.source_version_id)
        {
            if let Some(frame) = audio
                .frames
                .iter_mut()
                .find(|frame| frame.requested_ms == at)
            {
                if !frame.gaps.iter().any(|gap| gap == detail) {
                    frame.gaps.push(detail.into());
                }
            }
        }
        page.body = source_body(&page.info, "", None);
    } else if !page.body.contains(detail) {
        page.body
            .push_str(&format!("\n## Visual processing gap\n\n{detail}\n"));
    }
    if let Some(coverage) = page.info.extraction_coverage.as_mut() {
        for part in coverage.iter_mut().filter(|part| {
            matches!(
                part.scope,
                CoverageScope::ImageInterpretation | CoverageScope::VideoFrames
            )
        }) {
            part.status = CoverageStatus::Partial;
            part.detail = detail.into();
        }
    }
    page.info.extraction_detail = detail.into();
}
