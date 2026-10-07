//! Local English meaning retrieval. This module has no network-capable code;
//! the model, tokenizer, and native inference library must already be installed.
use ort::{
    session::{builder::GraphOptimizationLevel, Session},
    value::Tensor,
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tokenizers::Tokenizer;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

const DIMENSIONS: usize = 768;
const QUERY_PREFIX: &str = "Represent this sentence for searching relevant passages: ";
const MODEL_SHA256: &str = "9bc579acdba21c253c62a9bf866891355a63ffa3442b52c8a37d75b2ccb91848";
const TOKENIZER_SHA256: &str = "d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66";
const RUNTIME_SHA256: &str = "bcc9110f9d638a119de2db7afb3ba9a1da8085f0cb3401e1c48ae1caf450b6fa";
const RUNTIME_NAME: &str = "libonnxruntime.1.30.0.dylib";
const INDEX_IDENTITY: &str = "meaning-index/v1;BAAI/bge-base-en-v1.5@a5beb1e3e68b9ab74eb54cfd186867f64f240e1a;tokenizer=d241a60d5e8f04cc1b2b3e9ef7a4921b27bf526d9f6050ab90f9267a1f9e5c66;pool=normalized-cls;chunk=510";
// Selected using a separate three-query no-result calibration set. This is a
// local fixture cutoff, not a calibrated probability or general quality claim.
const MINIMUM_COSINE_SIMILARITY: f32 = 0.45;

static ORT_INIT: OnceLock<Result<(), String>> = OnceLock::new();

pub struct MeaningSearch {
    session: Session,
    tokenizer: Tokenizer,
    index_path: PathBuf,
    needs_rebuild: bool,
}

pub struct MeaningFilters<'a> {
    pub tags: &'a [String],
    pub date_from: Option<&'a str>,
    pub date_to: Option<&'a str>,
    pub format: Option<&'a str>,
    pub processing_status: Option<&'a str>,
}

impl MeaningSearch {
    pub fn open(
        assets_dir: impl AsRef<Path>,
        index_path: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let assets_dir = assets_dir.as_ref();
        let model_path = assets_dir.join("model.onnx");
        let tokenizer_path = assets_dir.join("tokenizer.json");
        let runtime_path = assets_dir.join(RUNTIME_NAME);
        let config_path = assets_dir.join("config.json");
        let tokenizer_config_path = assets_dir.join("tokenizer_config.json");
        let vocab_path = assets_dir.join("vocab.txt");
        for path in [
            &model_path,
            &tokenizer_path,
            &runtime_path,
            &config_path,
            &tokenizer_config_path,
            &vocab_path,
        ] {
            if !path.is_file() {
                return Err(format!(
                    "Meaning-search asset is missing: {}",
                    path.display()
                ));
            }
        }
        verify_sha(&model_path, MODEL_SHA256)?;
        verify_sha(&tokenizer_path, TOKENIZER_SHA256)?;
        verify_sha(&runtime_path, RUNTIME_SHA256)?;
        let runtime = runtime_path.clone();
        ORT_INIT
            .get_or_init(|| {
                if ort::init_from(runtime)
                    .map_err(|error| format!("Could not load local ONNX Runtime: {error}"))?
                    .commit()
                {
                    Ok(())
                } else {
                    Err("Could not initialize local ONNX Runtime.".into())
                }
            })
            .clone()?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|error| format!("The local meaning tokenizer is unreadable: {error}"))?;
        let mut session = Session::builder()
            .map_err(|error| format!("Could not prepare local meaning model: {error}"))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|error| format!("Could not configure local meaning model: {error}"))?
            .with_intra_threads(2)
            .map_err(|error| format!("Could not configure local meaning model: {error}"))?
            .with_inter_threads(1)
            .map_err(|error| format!("Could not configure local meaning model: {error}"))?
            .commit_from_file(&model_path)
            .map_err(|error| format!("Could not open the local meaning model: {error}"))?;
        if session
            .inputs()
            .iter()
            .map(|item| item.name())
            .collect::<Vec<_>>()
            != ["input_ids", "attention_mask", "token_type_ids"]
            || session.outputs().first().map(|item| item.name()) != Some("last_hidden_state")
        {
            return Err("The installed meaning model has an unsupported tensor layout.".into());
        }

        let index_path = index_path.as_ref().to_path_buf();
        if let Some(parent) = index_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let identity_path = index_path.with_extension("identity");
        let identity_matches =
            fs::read_to_string(&identity_path).is_ok_and(|value| value == INDEX_IDENTITY);
        let needs_rebuild = !identity_matches
            || !index_path.is_file()
            || Index::restore_view(&index_path.to_string_lossy()).is_err();
        if needs_rebuild {
            let _ = fs::remove_file(&index_path);
        }

        let _ = &mut session;
        Ok(Self {
            session,
            tokenizer,
            index_path,
            needs_rebuild,
        })
    }

    fn empty_index() -> Result<Index, String> {
        Index::new(&IndexOptions {
            dimensions: DIMENSIONS,
            metric: MetricKind::IP,
            quantization: ScalarKind::F32,
            ..Default::default()
        })
        .map_err(|error| format!("Could not create the local meaning index: {error}"))
    }

    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, String> {
        self.embed(&format!("{QUERY_PREFIX}{query}"))
    }

    pub fn embed_document(&mut self, text: &str) -> Result<Vec<f32>, String> {
        self.embed(text)
    }

    /// Incrementally reconcile vectors against current SQLite search documents.
    /// Markdown remains authoritative; this HNSW index and its lookup table are disposable.
    pub fn sync(&mut self, database: &Connection) -> Result<(), String> {
        database
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS meaning_vectors (
               vector_key INTEGER PRIMARY KEY,
               page_id TEXT NOT NULL,
               chunk_index INTEGER NOT NULL,
               content_hash TEXT NOT NULL,
               UNIQUE(page_id, chunk_index)
             );
             CREATE INDEX IF NOT EXISTS meaning_vectors_page ON meaning_vectors(page_id);",
            )
            .map_err(|error| format!("Could not prepare meaning-index metadata: {error}"))?;
        let index = if self.needs_rebuild || !self.index_path.is_file() {
            self.needs_rebuild = true;
            Self::empty_index()?
        } else {
            match Index::restore(&self.index_path.to_string_lossy()) {
                Ok(index) => index,
                Err(_) => {
                    self.needs_rebuild = true;
                    let _ = fs::remove_file(&self.index_path);
                    Self::empty_index()?
                }
            }
        };
        if self.needs_rebuild {
            database
                .execute("DELETE FROM meaning_vectors", [])
                .map_err(|error| {
                    format!("Could not reset stale meaning-index metadata: {error}")
                })?;
        }
        let mut statement = database.prepare(
            "SELECT page_id, title, kind, content FROM page_search WHERE doc_kind='page' ORDER BY page_id"
        ).map_err(|error| format!("Could not read current pages for meaning indexing: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (page_id, title, kind, content) = row.map_err(|error| error.to_string())?;
            let body = format!("{title}\n{kind}\n{content}");
            let digest = format!("{:x}", Sha256::digest(body.as_bytes()));
            let unchanged = database.query_row(
                "SELECT EXISTS(SELECT 1 FROM meaning_vectors WHERE page_id=?1 AND content_hash=?2)",
                params![page_id, digest], |row| row.get::<_, bool>(0),
            ).map_err(|error| error.to_string())?;
            if unchanged {
                continue;
            }

            let old_keys = {
                let mut query = database
                    .prepare("SELECT vector_key FROM meaning_vectors WHERE page_id=?1")
                    .map_err(|error| error.to_string())?;
                let rows = query
                    .query_map([&page_id], |row| row.get::<_, i64>(0))
                    .map_err(|error| error.to_string())?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                rows
            };
            for key in old_keys {
                let _ = index.remove(key as u64);
            }
            database
                .execute("DELETE FROM meaning_vectors WHERE page_id=?1", [&page_id])
                .map_err(|error| error.to_string())?;

            let chunks = self.document_chunks(&body)?;
            for (chunk_index, vector) in chunks.iter().enumerate() {
                let key = stable_vector_key(&page_id, chunk_index);
                if index.size() >= index.capacity() {
                    let next_capacity = index.capacity().saturating_mul(2).max(64);
                    index
                        .reserve(next_capacity)
                        .map_err(|error| format!("Could not grow local meaning index: {error}"))?;
                }
                index
                    .add(key, vector)
                    .map_err(|error| format!("Could not update local meaning index: {error}"))?;
                database.execute(
                    "INSERT INTO meaning_vectors(vector_key,page_id,chunk_index,content_hash) VALUES (?1,?2,?3,?4)",
                    params![key as i64, page_id, chunk_index as i64, digest],
                ).map_err(|error| format!("Could not save meaning-index metadata: {error}"))?;
            }
        }

        let stale_keys = {
            let mut query = database.prepare(
                "SELECT v.vector_key FROM meaning_vectors v LEFT JOIN page_search p ON p.page_id=v.page_id AND p.doc_kind='page' WHERE p.page_id IS NULL"
            ).map_err(|error| error.to_string())?;
            let rows = query
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?;
            rows
        };
        for key in stale_keys {
            let _ = index.remove(key as u64);
        }
        database.execute(
            "DELETE FROM meaning_vectors WHERE page_id NOT IN (SELECT page_id FROM page_search WHERE doc_kind='page')", [],
        ).map_err(|error| error.to_string())?;
        index
            .save(&self.index_path.to_string_lossy())
            .map_err(|error| format!("Could not save the local meaning index: {error}"))?;
        let identity_path = self.index_path.with_extension("identity");
        let temporary_identity = identity_path.with_extension("identity.tmp");
        fs::write(&temporary_identity, INDEX_IDENTITY)
            .and_then(|_| fs::rename(&temporary_identity, &identity_path))
            .map_err(|error| format!("Could not save meaning-index identity: {error}"))?;
        self.needs_rebuild = false;
        Ok(())
    }

    pub fn ranked_pages(
        &mut self,
        database: &Connection,
        query: &str,
        limit: usize,
        filters: Option<&MeaningFilters<'_>>,
    ) -> Result<Vec<(String, f32)>, String> {
        let vector = self.embed_query(query)?;
        let matches = self.search_vector(
            database,
            &vector,
            limit.saturating_mul(4).max(limit),
            filters,
        )?;
        let mut scores = HashMap::<String, f32>::new();
        for (key, score) in matches {
            let page_id = database
                .query_row(
                    "SELECT page_id FROM meaning_vectors WHERE vector_key=?1",
                    [key as i64],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            if let Some(page_id) = page_id {
                scores
                    .entry(page_id)
                    .and_modify(|best| *best = best.max(score))
                    .or_insert(score);
            }
        }
        let mut ranked = scores.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        ranked.retain(|(_, score)| *score >= MINIMUM_COSINE_SIMILARITY);
        ranked.truncate(limit);
        Ok(ranked)
    }

    fn search_vector(
        &self,
        database: &Connection,
        vector: &[f32],
        limit: usize,
        filters: Option<&MeaningFilters<'_>>,
    ) -> Result<Vec<(u64, f32)>, String> {
        if limit == 0 || !self.index_path.is_file() {
            return Ok(Vec::new());
        }
        let index = Index::restore_view(&self.index_path.to_string_lossy()).map_err(|error| {
            format!("Meaning index is unavailable and must be rebuilt: {error}")
        })?;
        if index.size() == 0 {
            return Ok(Vec::new());
        }
        let matches = match filters {
            Some(filters) => filtered_search(database, &index, vector, limit, filters)?,
            None => index
                .search(vector, limit)
                .map(|matches| matches.keys.into_iter().zip(matches.distances).collect())
                .map_err(|error| format!("Local meaning search failed: {error}"))?,
        };
        Ok(matches
            .into_iter()
            .map(|(key, distance)| (key, 1.0 - distance))
            .collect())
    }

    fn embed(&mut self, text: &str) -> Result<Vec<f32>, String> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|error| format!("Could not tokenize meaning-search text: {error}"))?;
        let ids = encoding
            .get_ids()
            .iter()
            .take(512)
            .map(|&value| value as i64)
            .collect::<Vec<_>>();
        self.embed_ids(&ids)
    }

    fn document_chunks(&mut self, text: &str) -> Result<Vec<Vec<f32>>, String> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|error| format!("Could not tokenize page for meaning indexing: {error}"))?;
        let ids = encoding.get_ids();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let cls = ids[0];
        let sep = *ids.last().unwrap();
        let body = if ids.len() > 2 {
            &ids[1..ids.len() - 1]
        } else {
            &[]
        };
        if body.is_empty() {
            return self.embed(text).map(|vector| vec![vector]);
        }
        body.chunks(510)
            .map(|part| {
                let mut ids = Vec::with_capacity(part.len() + 2);
                ids.push(cls);
                ids.extend_from_slice(part);
                ids.push(sep);
                ids.into_iter().map(i64::from).collect::<Vec<_>>()
            })
            .map(|ids| self.embed_ids(&ids))
            .collect()
    }

    fn embed_ids(&mut self, ids: &[i64]) -> Result<Vec<f32>, String> {
        if ids.is_empty() || ids.len() > 512 {
            return Err("Meaning model input must contain 1 to 512 tokens.".into());
        }
        let length = ids.len();
        let mask = vec![1_i64; length];
        let types = vec![0_i64; length];
        let outputs = self.session.run(ort::inputs![
            "input_ids" => Tensor::from_array(([1, length], ids.to_vec())).map_err(|error| error.to_string())?,
            "attention_mask" => Tensor::from_array(([1, length], mask)).map_err(|error| error.to_string())?,
            "token_type_ids" => Tensor::from_array(([1, length], types)).map_err(|error| error.to_string())?
        ]).map_err(|error| format!("Local meaning embedding failed: {error}"))?;
        let (shape, data) = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()
            .map_err(|error| format!("Local meaning model returned invalid vectors: {error}"))?;
        if shape.len() != 3
            || shape[0] != 1
            || shape[2] != DIMENSIONS as i64
            || data.len() < DIMENSIONS
        {
            return Err("Local meaning model returned an unsupported vector shape.".into());
        }
        let mut vector = data[..DIMENSIONS].to_vec(); // BGE's published CLS pooling.
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if !norm.is_finite() || norm <= f32::EPSILON {
            return Err("Local meaning model returned an empty vector.".into());
        }
        vector.iter_mut().for_each(|value| *value /= norm);
        Ok(vector)
    }
}

fn filtered_search(
    database: &Connection,
    index: &Index,
    vector: &[f32],
    limit: usize,
    filters: &MeaningFilters<'_>,
) -> Result<Vec<(u64, f32)>, String> {
    let mut conditions = vec![
        "v.vector_key = ?".to_owned(),
        "p.page_id = v.page_id".to_owned(),
        "p.doc_kind = 'page'".to_owned(),
    ];
    let mut values = vec![rusqlite::types::Value::Integer(0)];
    if !filters.tags.is_empty() {
        let placeholders = vec!["?"; filters.tags.len()].join(", ");
        conditions.push(format!(
            "EXISTS (SELECT 1 FROM page_tags t WHERE t.page_id = p.page_id AND t.normalized IN ({placeholders}))"
        ));
        values.extend(
            filters
                .tags
                .iter()
                .cloned()
                .map(rusqlite::types::Value::Text),
        );
    }
    if let Some(date_from) = filters.date_from {
        conditions.push("p.event_date IS NOT NULL AND p.event_date >= ?".into());
        values.push(date_from.to_owned().into());
    }
    if let Some(date_to) = filters.date_to {
        conditions.push("p.event_date IS NOT NULL AND p.event_date <= ?".into());
        values.push(date_to.to_owned().into());
    }
    if let Some(format) = filters.format {
        conditions.push("lower(p.format) = lower(?)".into());
        values.push(format.to_owned().into());
    }
    if let Some(status) = filters.processing_status {
        conditions.push("p.processing_status = ?".into());
        values.push(status.to_owned().into());
    }
    let sql = format!(
        "SELECT EXISTS(SELECT 1 FROM meaning_vectors v, page_search p WHERE {})",
        conditions.join(" AND ")
    );
    let statement = database
        .prepare(&sql)
        .map_err(|error| format!("Could not prepare meaning-search filters: {error}"))?;
    let predicate = RefCell::new((statement, values));
    let predicate_error = RefCell::new(None);
    let matches = index
        .filtered_search(vector, limit, |key| {
            let mut state = predicate.borrow_mut();
            let (statement, values) = &mut *state;
            values[0] = (key as i64).into();
            match statement.query_row(rusqlite::params_from_iter(values.iter()), |row| {
                row.get::<_, bool>(0)
            }) {
                Ok(eligible) => eligible,
                Err(error) => {
                    predicate_error.replace(Some(error.to_string()));
                    false
                }
            }
        })
        .map_err(|error| format!("Could not search filtered meaning vectors: {error}"))?;
    if let Some(error) = predicate_error.into_inner() {
        return Err(format!(
            "Could not evaluate meaning-search filters: {error}"
        ));
    }
    Ok(matches.keys.into_iter().zip(matches.distances).collect())
}

pub fn stable_vector_key(page_id: &str, chunk: usize) -> u64 {
    let mut digest = Sha256::new();
    digest.update(page_id.as_bytes());
    digest.update(b"\0meaning-chunk\0");
    digest.update(chunk.to_be_bytes());
    u64::from_be_bytes(
        digest.finalize()[..8]
            .try_into()
            .expect("digest prefix has fixed length"),
    )
}

fn verify_sha(path: &Path, expected: &str) -> Result<(), String> {
    use std::io::Read;
    let mut file = fs::File::open(path)
        .map_err(|error| format!("Could not read meaning asset {}: {error}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read meaning asset {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    let actual = format!("{:x}", digest.finalize());
    if actual != expected {
        return Err(format!(
            "Meaning asset checksum failed for {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{filtered_search, MeaningFilters};
    use rusqlite::{params, Connection};
    use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

    #[test]
    fn filtered_search_finds_eligible_page_beyond_250_unfiltered_neighbors() {
        let database = Connection::open_in_memory().unwrap();
        database
            .execute_batch(
                "CREATE TABLE meaning_vectors(vector_key INTEGER PRIMARY KEY, page_id TEXT);
                 CREATE TABLE page_search(page_id TEXT, doc_kind TEXT, event_date TEXT, format TEXT, processing_status TEXT);
                 CREATE TABLE page_tags(page_id TEXT, normalized TEXT);",
            )
            .unwrap();
        let index = Index::new(&IndexOptions {
            dimensions: 2,
            metric: MetricKind::IP,
            quantization: ScalarKind::F32,
            ..Default::default()
        })
        .unwrap();
        index.reserve(256).unwrap();
        let near = [1.0, 0.0];
        for key in 1..=251_u64 {
            let page_id = format!("page-ineligible-{key}");
            let angle = key as f32 * 0.00001;
            database
                .execute(
                    "INSERT INTO meaning_vectors(vector_key,page_id) VALUES (?1,?2)",
                    params![key as i64, page_id],
                )
                .unwrap();
            database
                .execute(
                    "INSERT INTO page_search(page_id,doc_kind,event_date,format,processing_status) VALUES (?1,'page','2023-10-02','txt','complete')",
                    [page_id],
                )
                .unwrap();
            index.add(key, &[angle.cos(), angle.sin()]).unwrap();
        }
        let eligible_key = 1000_u64;
        database
            .execute(
                "INSERT INTO meaning_vectors(vector_key,page_id) VALUES (?1,'page-eligible')",
                [eligible_key as i64],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO page_search(page_id,doc_kind,event_date,format,processing_status) VALUES ('page-eligible','page','2024-05-18','txt','complete')",
                [],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO page_tags(page_id,normalized) VALUES ('page-eligible','garden')",
                [],
            )
            .unwrap();
        index.add(eligible_key, &[0.8, 0.6]).unwrap();

        let unfiltered = index.exact_search(&near, 250).unwrap();
        assert_eq!(unfiltered.keys.len(), 250);
        assert!(unfiltered.keys.iter().all(|key| *key != eligible_key));

        let filters = MeaningFilters {
            tags: &["garden".to_owned()],
            date_from: Some("2024-05-18"),
            date_to: Some("2024-05-18"),
            format: Some("txt"),
            processing_status: Some("complete"),
        };
        let filtered = filtered_search(&database, &index, &near, 1, &filters).unwrap();
        assert_eq!(
            filtered.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            vec![eligible_key]
        );
    }
}
