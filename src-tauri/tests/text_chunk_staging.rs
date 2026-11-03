use knowledge_garden::semantic::KnowledgeDraft;
use knowledge_garden::text_chunk_staging::{checkpoint, commit, StagedChunk};

#[test]
fn staged_chunk_checkpoint_survives_reopen_and_retries() {
    let dir = tempfile::tempdir().unwrap();
    let job = dir.path().join("job");
    let chunk = StagedChunk {
        source_version_id: "version-a".into(),
        generation: "generation-7".into(),
        start: 0,
        end: 128,
        draft: KnowledgeDraft::default(),
    };
    commit(&job, &chunk, 256).unwrap();
    commit(&job, &chunk, 256).unwrap();
    assert_eq!(
        checkpoint(&job, "version-a", "generation-7", 256).unwrap(),
        128
    );
    assert!(checkpoint(&job, "version-b", "generation-7", 256).is_err());
    assert!(checkpoint(&job, "version-a", "generation-8", 256).is_err());
    let next = StagedChunk {
        start: 128,
        end: 256,
        ..chunk
    };
    commit(&job, &next, 256).unwrap();
    assert_eq!(
        checkpoint(&job, "version-a", "generation-7", 256).unwrap(),
        256
    );
}

#[test]
fn resumes_large_multibyte_source_after_interruption() {
    use knowledge_garden::text_chunks::read_chunk;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.txt");
    let job = dir.path().join("job");
    let text = format!(
        "{}🙂{}終",
        "x".repeat(2 * 1024 * 1024 - 1),
        "y".repeat(12345)
    );
    std::fs::write(&source, text.as_bytes()).unwrap();
    let total = text.len() as u64;
    let first = read_chunk(&source, 0, 65536).unwrap().unwrap();
    commit(
        &job,
        &StagedChunk {
            source_version_id: "version-a".into(),
            generation: "generation-1".into(),
            start: first.start,
            end: first.end,
            draft: KnowledgeDraft::default(),
        },
        total,
    )
    .unwrap();
    // Reconstruct the checkpoint from disk as a restarted worker would.
    let mut offset = checkpoint(&job, "version-a", "generation-1", total).unwrap();
    assert_eq!(offset, first.end);
    while let Some(chunk) = read_chunk(&source, offset, 65536).unwrap() {
        assert_eq!(
            &text.as_bytes()[chunk.start as usize..chunk.end as usize],
            chunk.text.as_bytes()
        );
        commit(
            &job,
            &StagedChunk {
                source_version_id: "version-a".into(),
                generation: "generation-1".into(),
                start: chunk.start,
                end: chunk.end,
                draft: KnowledgeDraft::default(),
            },
            total,
        )
        .unwrap();
        offset = chunk.end;
    }
    assert_eq!(
        checkpoint(&job, "version-a", "generation-1", total).unwrap(),
        total
    );
}

#[test]
fn cursor_rebuilds_from_records_after_interrupted_commit_and_corruption() {
    use knowledge_garden::text_chunk_staging::ChunkStore;
    let dir = tempfile::tempdir().unwrap();
    let job = dir.path().join("job");
    let first = StagedChunk {
        source_version_id: "v".into(),
        generation: "g".into(),
        start: 0,
        end: 100,
        draft: KnowledgeDraft::default(),
    };
    // The record exists but no cursor was written, as after a crash.
    commit(&job, &first, 200).unwrap();
    let mut store = ChunkStore::open(&job, "v", "g", 200).unwrap();
    assert_eq!(store.offset(), 100);
    let second = StagedChunk {
        start: 100,
        end: 200,
        ..first
    };
    store.commit(&second).unwrap();
    std::fs::write(job.join("cursor.json"), b"corrupt").unwrap();
    assert_eq!(ChunkStore::open(&job, "v", "g", 200).unwrap().offset(), 200);
    std::fs::remove_file(job.join(format!("{:020}.json", 0))).unwrap();
    assert_eq!(ChunkStore::open(&job, "v", "g", 200).unwrap().offset(), 0);
}

#[test]
fn active_store_does_not_rescan_previous_records_on_each_commit() {
    use knowledge_garden::text_chunk_staging::ChunkStore;
    let dir = tempfile::tempdir().unwrap();
    let job = dir.path().join("job");
    let mut store = ChunkStore::open(&job, "v", "g", 200).unwrap();
    let first = StagedChunk {
        source_version_id: "v".into(),
        generation: "g".into(),
        start: 0,
        end: 100,
        draft: KnowledgeDraft::default(),
    };
    store.commit(&first).unwrap();
    store.commit(&first).unwrap();
    std::fs::write(job.join(format!("{:020}.json", 0)), b"corrupt").unwrap();
    let second = StagedChunk {
        start: 100,
        end: 200,
        ..first
    };
    store.commit(&second).unwrap();
    assert!(ChunkStore::open(&job, "v", "g", 200).is_err());
}

#[test]
fn validates_and_rebases_chunk_evidence() {
    use knowledge_garden::semantic::{EntityDraft, EvidenceDraft};
    use knowledge_garden::text_chunk_staging::validate_and_rebase;
    let evidence = EvidenceDraft {
        quote: "終".into(),
        byte_start: 1,
        byte_end: 4,
        origin: "source".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let mut draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "topic".into(),
            label: "end".into(),
            evidence,
        }],
        ..Default::default()
    };
    validate_and_rebase(&mut draft, "x終", 100).unwrap();
    assert_eq!(
        (
            draft.entities[0].evidence.byte_start,
            draft.entities[0].evidence.byte_end
        ),
        (101, 104)
    );
    let mut invalid = draft.clone();
    invalid.entities[0].evidence.byte_start = 2;
    invalid.entities[0].evidence.byte_end = 4;
    assert!(validate_and_rebase(&mut invalid, "x終", 0).is_err());
}

#[test]
fn incomplete_chain_and_excessive_output_are_never_publishable() {
    use knowledge_garden::{
        semantic::{EntityDraft, EvidenceDraft},
        text_chunk_staging::aggregate,
    };
    let temp = tempfile::tempdir().unwrap();
    assert!(aggregate(temp.path(), "v", "g", 10).is_err());
    let evidence = EvidenceDraft {
        quote: "x".into(),
        byte_start: 0,
        byte_end: 1,
        origin: "source".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let mut draft = KnowledgeDraft::default();
    draft.entities = (0..501)
        .map(|i| EntityDraft {
            kind: "topic".into(),
            label: format!("topic{i}"),
            evidence: evidence.clone(),
        })
        .collect();
    knowledge_garden::text_chunk_staging::commit(
        temp.path(),
        &StagedChunk {
            source_version_id: "v".into(),
            generation: "g".into(),
            start: 0,
            end: 10,
            draft,
        },
        10,
    )
    .unwrap();
    assert!(aggregate(temp.path(), "v", "g", 10)
        .unwrap_err()
        .to_string()
        .contains("record budget"));
}

#[test]
fn aggregate_staging_budget_stops_before_advancing_the_checkpoint() {
    use knowledge_garden::semantic::{EntityDraft, EvidenceDraft};
    use knowledge_garden::text_chunk_staging::ChunkStore;
    let temp = tempfile::tempdir().unwrap();
    let evidence = EvidenceDraft {
        quote: "x".into(),
        byte_start: 0,
        byte_end: 1,
        origin: "source".into(),
        qualifier: None,
        offset_basis: None,
        source_location: None,
    };
    let draft = KnowledgeDraft {
        entities: vec![EntityDraft {
            kind: "topic".into(),
            label: "x".repeat(4 * 1024 * 1024 + 1),
            evidence,
        }],
        ..Default::default()
    };
    let mut store = ChunkStore::open(temp.path(), "v", "g", 20).unwrap();
    store
        .commit(&StagedChunk {
            source_version_id: "v".into(),
            generation: "g".into(),
            start: 0,
            end: 10,
            draft: draft.clone(),
        })
        .unwrap();
    assert!(store
        .commit(&StagedChunk {
            source_version_id: "v".into(),
            generation: "g".into(),
            start: 10,
            end: 20,
            draft
        })
        .is_err());
    assert_eq!(store.offset(), 10);
    assert_eq!(
        ChunkStore::open(temp.path(), "v", "g", 20)
            .unwrap()
            .offset(),
        10
    );
}
