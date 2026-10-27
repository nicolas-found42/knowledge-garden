//! Exact Boolean operations over the collection's disposable indexed projection.
use super::*;
const CURSOR: &str = "kg-boolean-v1:";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueryConstraint {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub format: Option<String>,
    pub processing_status: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHop {
    pub kind: String,
    pub direction: TraversalDirection,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum BooleanExpression {
    Path {
        start_page_id: String,
        hops: Vec<QueryHop>,
        #[serde(default)]
        endpoint: QueryConstraint,
        #[serde(default)]
        intermediate: QueryConstraint,
    },
    Constraint {
        constraint: QueryConstraint,
    },
    And {
        children: Vec<BooleanExpression>,
    },
    Or {
        children: Vec<BooleanExpression>,
    },
    Not {
        child: Box<BooleanExpression>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanQueryRequest {
    pub expression: BooleanExpression,
    #[serde(default)]
    pub returned: QueryConstraint,
    pub continuation: Option<String>,
    pub work_budget: usize,
    pub page_size: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanQueryResult {
    pub page_id: String,
    pub source_id: String,
    pub page_type: String,
    pub title: String,
    pub kind: String,
    pub paths: Vec<ExploredPath>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BooleanQueryResponse {
    pub results: Vec<BooleanQueryResult>,
    pub graph_revision: String,
    pub next_cursor: Option<String>,
    pub complete: bool,
    pub work_units: usize,
    pub diagnostics: Vec<PathExploreDiagnostic>,
}
#[derive(Clone)]
struct PathClause {
    start: String,
    hops: Vec<QueryHop>,
    endpoint: QueryConstraint,
    intermediate: QueryConstraint,
}
#[derive(Debug)]
struct PageMetadata {
    page_id: String,
    source_id: String,
    page_type: String,
    title: String,
    kind: String,
    date: Option<String>,
    format: String,
    status: String,
}
impl QueryConstraint {
    fn validate(&self) -> Result<()> {
        for date in [&self.date_from, &self.date_to].into_iter().flatten() {
            validate_iso_date(date)?;
        }
        if self
            .date_from
            .as_ref()
            .zip(self.date_to.as_ref())
            .is_some_and(|(from, to)| from > to)
        {
            return Err(GardenError::Invalid(
                "The start date must not be after the end date.".into(),
            ));
        }
        if self.processing_status.as_deref().is_some_and(|s| {
            ![
                "pending",
                "processing",
                "complete",
                "failed",
                "unavailable",
                "update_pending",
                "update_failed",
                "identity_ambiguous",
                "extraction_incomplete",
                "source_unavailable",
            ]
            .contains(&s)
        }) {
            return Err(GardenError::Invalid("Unknown processing status.".into()));
        }
        Ok(())
    }
    fn matches(&self, page: &PageMetadata) -> bool {
        self.date_from
            .as_ref()
            .is_none_or(|from| page.date.as_ref().is_some_and(|d| d >= from))
            && self
                .date_to
                .as_ref()
                .is_none_or(|to| page.date.as_ref().is_some_and(|d| d <= to))
            && self.format.as_ref().is_none_or(|f| f == &page.format)
            && self
                .processing_status
                .as_ref()
                .is_none_or(|s| s == &page.status)
    }
}
fn clauses(
    expression: &BooleanExpression,
    paths: &mut Vec<PathClause>,
    count: &mut usize,
    depth: usize,
) -> Result<()> {
    *count += 1;
    if *count > 64 || depth > 16 {
        return Err(GardenError::Invalid(
            "A query supports up to 64 expression nodes (including groups and clauses) and 16 nested groups.".into(),
        ));
    }
    match expression {
        BooleanExpression::Path {
            start_page_id,
            hops,
            endpoint,
            intermediate,
        } => {
            if hops.is_empty()
                || hops.len() > 32
                || hops
                    .iter()
                    .any(|h| h.kind.trim().is_empty() || h.kind.len() > 128)
            {
                return Err(GardenError::Invalid(
                    "A typed path needs 1 to 32 named relationship hops.".into(),
                ));
            }
            endpoint.validate()?;
            intermediate.validate()?;
            paths.push(PathClause {
                start: start_page_id.clone(),
                hops: hops.clone(),
                endpoint: endpoint.clone(),
                intermediate: intermediate.clone(),
            });
        }
        BooleanExpression::Constraint { constraint } => constraint.validate()?,
        BooleanExpression::And { children } | BooleanExpression::Or { children } => {
            for child in children {
                clauses(child, paths, count, depth + 1)?;
            }
        }
        BooleanExpression::Not { child } => clauses(child, paths, count, depth + 1)?,
    }
    Ok(())
}
fn page_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PageMetadata> {
    Ok(PageMetadata {
        page_id: row.get(0)?,
        source_id: row.get(1)?,
        page_type: row.get(2)?,
        title: row.get(3)?,
        kind: row.get(4)?,
        date: row.get(5)?,
        format: row.get(6)?,
        status: row.get(7)?,
    })
}
pub(super) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS query_page_metadata(page_id TEXT PRIMARY KEY, source_id TEXT NOT NULL, page_type TEXT NOT NULL, title TEXT NOT NULL, kind TEXT NOT NULL, event_date TEXT, format TEXT NOT NULL, processing_status TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS boolean_queries(query_id TEXT PRIMARY KEY, signature TEXT NOT NULL, last_page_id TEXT NOT NULL DEFAULT '', canceled INTEGER NOT NULL DEFAULT 0);
CREATE INDEX IF NOT EXISTS boolean_queries_canceled ON boolean_queries(canceled,query_id);
CREATE TABLE IF NOT EXISTS boolean_frontier(query_id TEXT NOT NULL, leaf INTEGER NOT NULL, sequence INTEGER PRIMARY KEY AUTOINCREMENT, page_id TEXT NOT NULL, depth INTEGER NOT NULL, last_edge TEXT NOT NULL DEFAULT '', node_id TEXT);
CREATE INDEX IF NOT EXISTS boolean_frontier_order ON boolean_frontier(query_id, sequence);
CREATE TABLE IF NOT EXISTS boolean_visited(query_id TEXT NOT NULL, leaf INTEGER NOT NULL, page_id TEXT NOT NULL, depth INTEGER NOT NULL, PRIMARY KEY(query_id,leaf,page_id,depth));
CREATE TABLE IF NOT EXISTS boolean_matches(query_id TEXT NOT NULL, leaf INTEGER NOT NULL, page_id TEXT NOT NULL, node_id TEXT NOT NULL, PRIMARY KEY(query_id,leaf,page_id));
";
impl Application {
    fn boolean_page_metadata(&self, id: &str) -> Result<Option<PageMetadata>> {
        Ok(self.index.query_row("SELECT page_id,source_id,page_type,title,kind,event_date,format,processing_status FROM query_page_metadata WHERE page_id=?1",[id],page_row).optional()?)
    }
    pub fn cancel_query(&mut self, continuation: &str) -> Result<()> {
        let id = parse_opaque_path_token(continuation, CURSOR)?;
        self.index.execute(
            "UPDATE boolean_queries SET canceled=1 WHERE query_id=?1",
            [id],
        )?;
        Ok(())
    }
    pub fn query_pages(&mut self, request: BooleanQueryRequest) -> Result<BooleanQueryResponse> {
        if !(1..=MAX_PATH_WORK_BUDGET).contains(&request.work_budget)
            || !(1..=PAGE_SIZE).contains(&request.page_size)
        {
            return Err(GardenError::Invalid(
                "Choose a bounded query budget and result page size.".into(),
            ));
        }
        request.returned.validate()?;
        let mut paths = Vec::new();
        clauses(&request.expression, &mut paths, &mut 0, 0)?;
        let signature = serde_json::to_string(&(&request.expression, &request.returned))?;
        let revision = self.path_graph_revision()?;
        let now = i64::try_from(now_millis()?)
            .map_err(|_| GardenError::Invalid("Invalid query clock.".into()))?;
        let id = if let Some(cursor) = request.continuation.as_deref() {
            let id = parse_opaque_path_token(cursor, CURSOR)?;
            let (stored,old_revision,updated,canceled):(String,String,i64,bool)=self.index.query_row("SELECT b.signature,q.graph_revision,q.updated_at,b.canceled FROM boolean_queries b JOIN path_exploration_queries q USING(query_id) WHERE b.query_id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?.ok_or_else(||GardenError::Invalid("Boolean query cursor expired or is invalid.".into()))?;
            if canceled {
                return Err(GardenError::Invalid("Boolean query was canceled.".into()));
            }
            if old_revision != revision {
                return Err(GardenError::Invalid(
                    "Boolean query cursor is stale because the collection changed.".into(),
                ));
            }
            if updated < now.saturating_sub(PATH_QUERY_IDLE_TTL_MS) {
                return Err(GardenError::Invalid("Boolean query cursor expired.".into()));
            }
            if stored != signature {
                return Err(GardenError::Invalid(
                    "Boolean query cursor does not match the expression and scopes.".into(),
                ));
            }
            id
        } else {
            let id = Uuid::new_v4().to_string();
            self.index.execute("INSERT INTO path_exploration_queries(query_id,graph_revision,start_page_id,max_hops,next_sequence,updated_at) VALUES(?1,?2,'',32,0,?3)",params![id,revision,now])?;
            self.index.execute(
                "INSERT INTO boolean_queries(query_id,signature) VALUES(?1,?2)",
                params![id, signature],
            )?;
            for (leaf, path) in paths.iter().enumerate() {
                if !self.index.query_row(
                    "SELECT EXISTS(SELECT 1 FROM knowledge_pages WHERE page_id=?1)",
                    [&path.start],
                    |r| r.get::<_, bool>(0),
                )? {
                    return Err(GardenError::Invalid(
                        "A query starting page is unavailable.".into(),
                    ));
                }
                self.index.execute(
                    "INSERT INTO boolean_frontier(query_id,leaf,page_id,depth) VALUES(?1,?2,?3,0)",
                    params![id, leaf as i64, path.start],
                )?;
                self.index.execute(
                    "INSERT INTO boolean_visited VALUES(?1,?2,?3,0)",
                    params![id, leaf as i64, path.start],
                )?;
            }
            id
        };
        // Retire abandoned state in bounded pieces. Never load a previous query's frontier.
        let cutoff = now.saturating_sub(PATH_QUERY_IDLE_TTL_MS);
        for table in ["boolean_frontier", "boolean_visited", "boolean_matches"] {
            self.index.execute(&format!("DELETE FROM {table} WHERE rowid IN (SELECT t.rowid FROM {table} t WHERE t.query_id IN (SELECT query_id FROM boolean_queries WHERE canceled=1 UNION SELECT q.query_id FROM path_exploration_queries q JOIN boolean_queries b USING(query_id) WHERE q.updated_at<?1 LIMIT 50) LIMIT 50)"), [cutoff])?;
        }
        self.index.execute("DELETE FROM boolean_queries WHERE query_id IN (SELECT b.query_id FROM boolean_queries b WHERE b.query_id IN (SELECT query_id FROM boolean_queries WHERE canceled=1 UNION SELECT q.query_id FROM path_exploration_queries q JOIN boolean_queries b USING(query_id) WHERE q.updated_at<?1 LIMIT 50) AND NOT EXISTS(SELECT 1 FROM boolean_frontier f WHERE f.query_id=b.query_id) AND NOT EXISTS(SELECT 1 FROM boolean_visited v WHERE v.query_id=b.query_id) AND NOT EXISTS(SELECT 1 FROM boolean_matches m WHERE m.query_id=b.query_id) LIMIT 50)", [cutoff])?;
        prune_expired_path_state(&self.index, cutoff)?;
        self.index.execute(
            "UPDATE path_exploration_queries SET updated_at=?1 WHERE query_id=?2",
            params![now, id],
        )?;
        let mut work_units = 0;
        let mut results = Vec::new();
        let mut diagnostics = Vec::new();
        while work_units < request.work_budget && results.len() < request.page_size {
            work_units += 1;
            let frontier=self.index.query_row("SELECT sequence,leaf,page_id,depth,last_edge,node_id FROM boolean_frontier WHERE query_id=?1 ORDER BY sequence LIMIT 1",[&id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?))).optional()?;
            if let Some((sequence, leaf, page_id, depth, last_edge, parent)) = frontier {
                let path = &paths[leaf as usize];
                let hop = &path.hops[depth as usize];
                let sql = if hop.direction == TraversalDirection::WithArrow {
                    "SELECT relationship_id,from_page_id,to_page_id,relationship_json FROM relationship_edges WHERE from_page_id=?1 AND relationship_id>?2 ORDER BY relationship_id LIMIT 1"
                } else {
                    "SELECT relationship_id,from_page_id,to_page_id,relationship_json FROM relationship_edges WHERE to_page_id=?1 AND relationship_id>?2 ORDER BY relationship_id LIMIT 1"
                };
                let edge = self
                    .index
                    .query_row(sql, params![page_id, last_edge], path_edge_row)
                    .optional()?;
                let Some((edge_id, from, to, serialized)) = edge else {
                    self.index
                        .execute("DELETE FROM boolean_frontier WHERE sequence=?1", [sequence])?;
                    continue;
                };
                self.index.execute(
                    "UPDATE boolean_frontier SET last_edge=?1 WHERE sequence=?2",
                    params![edge_id, sequence],
                )?;
                let relation: RelationshipRecord = serde_json::from_str(&serialized)?;
                if relation.kind != hop.kind {
                    continue;
                }
                let target = if hop.direction == TraversalDirection::WithArrow {
                    &to
                } else {
                    &from
                };
                let Some(metadata) = self.boolean_page_metadata(target)? else {
                    diagnostics.push(PathExploreDiagnostic {
                        kind: PathExploreDiagnosticKind::MissingEndpoint,
                        relationship_id: edge_id,
                        endpoint_page_id: target.clone(),
                    });
                    continue;
                };
                let Some(support) = self.latest_relationship_support(&relation.supports)? else {
                    diagnostics.push(PathExploreDiagnostic {
                        kind: PathExploreDiagnosticKind::MissingSupport,
                        relationship_id: edge_id,
                        endpoint_page_id: target.clone(),
                    });
                    continue;
                };
                let Some((asset, current)) = self.path_support_asset(support)? else {
                    diagnostics.push(PathExploreDiagnostic {
                        kind: PathExploreDiagnosticKind::MissingSupport,
                        relationship_id: edge_id,
                        endpoint_page_id: target.clone(),
                    });
                    continue;
                };
                let final_hop = depth as usize + 1 == path.hops.len();
                if !(if final_hop {
                    &path.endpoint
                } else {
                    &path.intermediate
                })
                .matches(&metadata)
                {
                    continue;
                }
                let inserted = self.index.execute(
                    "INSERT OR IGNORE INTO boolean_visited VALUES(?1,?2,?3,?4)",
                    params![id, leaf, target, depth + 1],
                )?;
                if inserted == 0 {
                    continue;
                }
                let (basis, lines, bytes, guidance) =
                    evidence_display_basis(support.evidence.offset_basis);
                let step = ExploredPathStep {
                    relationship_id: edge_id,
                    from_page_id: from.clone(),
                    from_title: self.path_page_title(&from)?,
                    to_page_id: to.clone(),
                    to_title: self.path_page_title(&to)?,
                    kind: relation.kind,
                    qualifier: relation.qualifier,
                    traversal_direction: hop.direction,
                    evidence_quote: support.evidence.quote.clone(),
                    evidence_location: format!(
                        "{basis} · {lines} {}–{} · {bytes} {}–{}{}",
                        support.evidence.line_start,
                        support.evidence.line_end,
                        support.evidence.byte_start,
                        support.evidence.byte_end,
                        support
                            .evidence
                            .source_location
                            .as_ref()
                            .map(|s| format!(" · {s}"))
                            .unwrap_or_default()
                    ),
                    evidence_origin: support.evidence.origin.clone(),
                    evidence_qualifier: support.evidence.qualifier.clone(),
                    original_guidance: guidance.into(),
                    source_id: support.source_id.clone(),
                    source_version_id: support.source_version_id.clone(),
                    original_asset: asset,
                    source_is_current: current,
                };
                let node_id = Uuid::new_v4().to_string();
                self.index.execute("INSERT INTO path_query_nodes(query_id,node_id,parent_node_id,page_id,depth,step_json) VALUES(?1,?2,?3,?4,?5,?6)",params![id,node_id,parent,target,depth+1,serde_json::to_string(&step)?])?;
                if final_hop {
                    self.index.execute(
                        "INSERT OR IGNORE INTO boolean_matches VALUES(?1,?2,?3,?4)",
                        params![id, leaf, target, node_id],
                    )?;
                } else {
                    self.index.execute("INSERT INTO boolean_frontier(query_id,leaf,page_id,depth,node_id) VALUES(?1,?2,?3,?4,?5)",params![id,leaf,target,depth+1,node_id])?;
                }
                continue;
            }
            let last: String = self.index.query_row(
                "SELECT last_page_id FROM boolean_queries WHERE query_id=?1",
                [&id],
                |r| r.get(0),
            )?;
            let candidate=self.index.query_row("SELECT page_id,source_id,page_type,title,kind,event_date,format,processing_status FROM query_page_metadata WHERE page_id>?1 ORDER BY page_id LIMIT 1",[last],page_row).optional()?;
            let Some(page) = candidate else {
                break;
            };
            self.index.execute(
                "UPDATE boolean_queries SET last_page_id=?1 WHERE query_id=?2",
                params![page.page_id, id],
            )?;
            if !request.returned.matches(&page) {
                continue;
            }
            let (matches, leaves) =
                self.boolean_matches_expression(&id, &request.expression, &page, &mut 0, false)?;
            if !matches {
                continue;
            }
            let mut supporting_paths = Vec::new();
            for leaf in leaves {
                let node_id:String=self.index.query_row("SELECT node_id FROM boolean_matches WHERE query_id=?1 AND leaf=?2 AND page_id=?3",params![id,leaf as i64,page.page_id],|r|r.get(0))?;
                let token = Uuid::new_v4().to_string();
                self.index.execute("INSERT INTO path_detail_sessions(detail_token,query_id,target_page_id,target_title,current_node_id,cursor_token,complete,updated_at) VALUES(?1,?2,?3,?4,?5,NULL,0,?6)",params![token,id,page.page_id,page.title,node_id,now])?;
                supporting_paths.push(ExploredPath {
                    target_page_id: page.page_id.clone(),
                    target_title: page.title.clone(),
                    target_kind: page.kind.clone(),
                    detail_token: token,
                    step_count: paths[leaf].hops.len(),
                });
            }
            results.push(BooleanQueryResult {
                page_id: page.page_id,
                source_id: page.source_id,
                page_type: page.page_type,
                title: page.title,
                kind: page.kind,
                paths: supporting_paths,
            });
        }
        let remaining:bool=self.index.query_row("SELECT EXISTS(SELECT 1 FROM boolean_frontier WHERE query_id=?1) OR EXISTS(SELECT 1 FROM query_page_metadata WHERE page_id>(SELECT last_page_id FROM boolean_queries WHERE query_id=?1))",[&id],|r|r.get(0))?;
        Ok(BooleanQueryResponse {
            results,
            graph_revision: revision,
            next_cursor: remaining.then(|| format!("{CURSOR}{id}")),
            complete: !remaining,
            work_units,
            diagnostics,
        })
    }
    fn boolean_matches_expression(
        &self,
        id: &str,
        expression: &BooleanExpression,
        page: &PageMetadata,
        ordinal: &mut usize,
        negated: bool,
    ) -> Result<(bool, Vec<usize>)> {
        match expression {
            BooleanExpression::Path { .. } => {
                let leaf = *ordinal;
                *ordinal += 1;
                let found=self.index.query_row("SELECT EXISTS(SELECT 1 FROM boolean_matches WHERE query_id=?1 AND leaf=?2 AND page_id=?3)",params![id,leaf as i64,page.page_id],|r|r.get::<_,bool>(0))?;
                Ok((
                    found != negated,
                    if found && !negated {
                        vec![leaf]
                    } else {
                        vec![]
                    },
                ))
            }
            BooleanExpression::Constraint { constraint } => {
                Ok((constraint.matches(page) != negated, vec![]))
            }
            BooleanExpression::Not { child } => {
                self.boolean_matches_expression(id, child, page, ordinal, !negated)
            }
            BooleanExpression::And { children } | BooleanExpression::Or { children } => {
                // Push NOT through groups (De Morgan) so even-negated positive
                // paths keep their evidence, while absence never creates a path.
                let conjunction = matches!(expression, BooleanExpression::And { .. }) != negated;
                let mut matched = conjunction;
                let mut leaves = Vec::new();
                for child in children {
                    let (found, child_leaves) =
                        self.boolean_matches_expression(id, child, page, ordinal, negated)?;
                    if conjunction {
                        matched &= found;
                    } else {
                        matched |= found;
                    }
                    if found {
                        leaves.extend(child_leaves);
                    }
                }
                Ok((matched, if matched { leaves } else { vec![] }))
            }
        }
    }
}
