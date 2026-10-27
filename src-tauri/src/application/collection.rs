//! Bounded collection organization derived from current, evidence-backed pages.
use super::{Application, GardenError, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionAxis {
    #[default]
    Topic,
    Person,
    Place,
    Date,
    Format,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionGroupRequest {
    #[serde(default)]
    pub axis: CollectionAxis,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub offset: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionGroup {
    pub group_id: String,
    pub label: String,
    pub members: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionGroups {
    pub groups: Vec<CollectionGroup>,
    pub next_offset: Option<usize>,
    pub revision: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMemberRequest {
    pub group_id: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default)]
    pub revision: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionItem {
    pub page_id: String,
    pub source_id: String,
    pub page_type: String,
    pub title: String,
    pub kind: String,
    pub format: String,
    pub processing_status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMembers {
    pub items: Vec<CollectionItem>,
    pub next_offset: Option<usize>,
    pub revision: String,
    pub changed: bool,
}
const PAGE: usize = 50;

impl Application {
    fn organization_revision(&self) -> Result<String> {
        Ok(self.index.query_row(
            "SELECT revision FROM collection_group_meta WHERE singleton=1",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn collection_groups(&self, request: CollectionGroupRequest) -> Result<CollectionGroups> {
        if request.query.len() > 512 {
            return Err(GardenError::Invalid(
                "Collection filter is too long.".into(),
            ));
        }
        let offset = i64::try_from(request.offset)
            .map_err(|_| GardenError::Invalid("Invalid collection offset.".into()))?;
        let query = match request.axis {
            CollectionAxis::Topic => "SELECT 'subject:'||p.page_id AS id,p.title AS label,1+(SELECT COUNT(*) FROM collection_source_pages s WHERE s.knowledge_page_id=p.page_id) AS members FROM query_page_metadata p WHERE p.page_type='knowledge' UNION ALL SELECT 'tag:'||normalized,MIN(label),COUNT(DISTINCT page_id) FROM page_tags GROUP BY normalized UNION ALL SELECT 'unclassified','Unclassified or processing',COUNT(*) FROM query_page_metadata p WHERE p.page_type='source' AND NOT EXISTS(SELECT 1 FROM collection_source_pages s WHERE s.source_page_id=p.page_id) HAVING COUNT(*)>0",
            CollectionAxis::Person => "SELECT 'subject:'||p.page_id AS id,p.title AS label,1+(SELECT COUNT(*) FROM collection_source_pages s WHERE s.knowledge_page_id=p.page_id) AS members FROM query_page_metadata p WHERE p.page_type='knowledge' AND p.kind='person'",
            CollectionAxis::Place => "SELECT 'subject:'||p.page_id AS id,p.title AS label,1+(SELECT COUNT(*) FROM collection_source_pages s WHERE s.knowledge_page_id=p.page_id) AS members FROM query_page_metadata p WHERE p.page_type='knowledge' AND p.kind='place'",
            CollectionAxis::Date => "SELECT 'date:'||COALESCE(event_date,'' ) AS id,COALESCE(event_date,'Unknown event date') AS label,COUNT(*) AS members FROM query_page_metadata GROUP BY event_date",
            CollectionAxis::Format => "SELECT 'format:'||format AS id,UPPER(format) AS label,COUNT(*) AS members FROM query_page_metadata GROUP BY format",
        };
        let sql = format!("SELECT id,label,members FROM ({query}) WHERE instr(lower(label),lower(?1))>0 ORDER BY lower(label),id LIMIT ?2 OFFSET ?3");
        let mut stmt = self.index.prepare(&sql)?;
        let mut groups = stmt
            .query_map(params![request.query, (PAGE + 1) as i64, offset], |r| {
                Ok(CollectionGroup {
                    group_id: r.get(0)?,
                    label: r.get(1)?,
                    members: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_offset = if groups.len() > PAGE {
            groups.pop();
            Some(request.offset + PAGE)
        } else {
            None
        };
        Ok(CollectionGroups {
            groups,
            next_offset,
            revision: self.organization_revision()?,
        })
    }

    pub fn collection_members(
        &self,
        request: CollectionMemberRequest,
    ) -> Result<CollectionMembers> {
        if request.group_id.len() > 1024 {
            return Err(GardenError::Invalid("Collection group is too long.".into()));
        }
        let revision = self.organization_revision()?;
        let changed = request.revision.as_ref().is_some_and(|r| r != &revision);
        let offset = if changed { 0 } else { request.offset };
        let offset_i64 = i64::try_from(offset)
            .map_err(|_| GardenError::Invalid("Invalid collection offset.".into()))?;
        let (kind, value) = request
            .group_id
            .split_once(':')
            .unwrap_or((&request.group_id, ""));
        let predicate = match kind {
            "subject" => "p.page_id=?1 OR p.page_id IN (SELECT source_page_id FROM collection_source_pages WHERE knowledge_page_id=?1)",
            "source" => "p.source_id=?1 AND p.page_type='source' OR p.page_id IN (SELECT knowledge_page_id FROM collection_source_pages WHERE source_id=?1)",
            "tag" => "p.page_id IN (SELECT page_id FROM page_tags WHERE normalized=?1)",
            "date" => "COALESCE(p.event_date,'')=?1",
            "format" => "p.format=?1",
            "unclassified" => "p.page_type='source' AND NOT EXISTS (SELECT 1 FROM collection_source_pages s WHERE s.source_page_id=p.page_id) AND ?1=''",
            _ => return Err(GardenError::Invalid("Unknown collection group.".into())),
        };
        let sql = format!("SELECT p.page_id,p.source_id,p.page_type,p.title,p.kind,p.format,p.processing_status FROM query_page_metadata p WHERE ({predicate}) ORDER BY lower(p.title),p.page_id LIMIT ?2 OFFSET ?3");
        let mut stmt = self.index.prepare(&sql)?;
        let mut items = stmt
            .query_map(params![value, (PAGE + 1) as i64, offset_i64], |r| {
                Ok(CollectionItem {
                    page_id: r.get(0)?,
                    source_id: r.get(1)?,
                    page_type: r.get(2)?,
                    title: r.get(3)?,
                    kind: r.get(4)?,
                    format: r.get(5)?,
                    processing_status: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_offset = if items.len() > PAGE {
            items.pop();
            Some(offset + PAGE)
        } else {
            None
        };
        Ok(CollectionMembers {
            items,
            next_offset,
            revision,
            changed,
        })
    }
}

/// Content identity stays stable across cache rebuilds and changes with membership/labels.
pub(super) fn index_revision(
    transaction: &rusqlite::Transaction<'_>,
    graph_revision: &str,
) -> Result<()> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(graph_revision.as_bytes());
    for (sql, columns) in [("SELECT source_page_id,source_id,knowledge_page_id FROM collection_source_pages ORDER BY source_page_id,knowledge_page_id",3),("SELECT page_id,normalized,label,source_id,source_version_id,support_json FROM page_tags ORDER BY page_id,normalized,source_id,source_version_id,support_json",6)] {
        let mut stmt=transaction.prepare(sql)?;let mut rows=stmt.query([])?;
        while let Some(row)=rows.next()? { for column in 0..columns { let value:String=row.get(column)?;digest.update((value.len() as u64).to_be_bytes());digest.update(value.as_bytes()); } }
    }
    transaction.execute(
        "UPDATE collection_group_meta SET revision=?1 WHERE singleton=1",
        [hex::encode(digest.finalize())],
    )?;
    Ok(())
}

/// Change only a source's affected memberships; an unchanged source does no group writes.
pub(super) fn index_source(
    transaction: &rusqlite::Transaction<'_>,
    info: &super::SourceInfo,
) -> Result<()> {
    use std::collections::BTreeSet;
    let desired = info
        .knowledge_pages
        .iter()
        .map(|page| (info.page_id.clone(), page.page_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut stmt = transaction.prepare(
        "SELECT source_page_id,knowledge_page_id FROM collection_source_pages WHERE source_id=?1",
    )?;
    let known = stmt
        .query_map([&info.source_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<BTreeSet<_>>>()?;
    if desired != known {
        transaction.execute(
            "DELETE FROM collection_source_pages WHERE source_id=?1",
            [&info.source_id],
        )?;
        for (source_page, knowledge_page) in desired {
            transaction.execute(
                "INSERT INTO collection_source_pages VALUES(?1,?2,?3)",
                params![source_page, info.source_id, knowledge_page],
            )?;
        }
    }
    Ok(())
}
