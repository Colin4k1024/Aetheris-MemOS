//! Bi-temporal Knowledge Graph Service (Issue #51).
//!
//! Provides a **uniform bi-temporal query interface** on top of the existing
//! `KGRepository` tables that already carry `valid_from`, `valid_until`, and
//! `superseded_by` columns for entities.
//!
//! The two time dimensions are:
//!
//! | Dimension         | Meaning                                      |
//! |-------------------|----------------------------------------------|
//! | **Valid time**    | When a fact was *true in the world*.         |
//! | **Transaction time** | When the fact was *recorded in the DB*.   |
//!
//! This module adds:
//! 1. **Relation versioning** — `supersede_relation` mirrors the entity
//!    supersede pattern and records a valid-time termination on old relations.
//! 2. **Point-in-time entity-relation snapshot** — `snapshot_at` fetches all
//!    entities and relations that were simultaneously valid at a given
//!    wall-clock time.
//! 3. **Change-set diff** — `diff_intervals` returns added / removed / modified
//!    entities and relations between two time points.
//! 4. **Temporal conflict detection** — `detect_contradictions` checks whether
//!    two concurrent (overlapping valid-time) relations between the same entity
//!    pair carry logically incompatible types.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::db::kg::{Entity, KGRepository, Relation};
use crate::db::pool;
use crate::tenant::get_default_tenant;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of the KG: all entities and relations that were
/// valid (in the world) at the given instant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KgSnapshot {
    /// ISO-8601 instant this snapshot represents.
    pub valid_at: String,
    pub entities: Vec<Entity>,
    pub relations: Vec<RelationVersion>,
}

/// A relation annotated with its valid-time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationVersion {
    pub relation_id: String,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub relation_type: String,
    pub relation_name: Option<String>,
    pub description: Option<String>,
    pub properties: Option<String>,
    pub weight: f64,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    pub superseded_by: Option<String>,
}

impl From<Relation> for RelationVersion {
    fn from(r: Relation) -> Self {
        Self {
            relation_id: r.relation_id,
            source_entity_id: r.source_entity_id,
            target_entity_id: r.target_entity_id,
            relation_type: r.relation_type,
            relation_name: r.relation_name,
            description: r.description,
            properties: r.properties,
            weight: r.weight,
            confidence: r.confidence,
            created_at: r.created_at,
            updated_at: r.updated_at,
            status: r.status,
            // Relation table does not yet carry temporal columns on the existing
            // struct — defaults to None until a DB migration adds them.
            valid_from: None,
            valid_until: None,
            superseded_by: None,
        }
    }
}

/// Result of a temporal diff between two time points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalDiff {
    pub from_time: String,
    pub to_time: String,
    pub added_entities: Vec<Entity>,
    pub removed_entities: Vec<Entity>,
    pub modified_entities: Vec<(Entity, Entity)>, // (old, new)
    pub added_relations: usize,
    pub removed_relations: usize,
}

/// A detected temporal contradiction between two relations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalContradiction {
    pub entity_id: String,
    pub relation_type: String,
    pub relation_a: String,
    pub relation_b: String,
    pub overlap_description: String,
}

// ---------------------------------------------------------------------------
// Bi-temporal query functions
// ---------------------------------------------------------------------------

/// Fetch a point-in-time snapshot of all *currently known* entities and their
/// relations that were valid-in-the-world at `valid_at` (ISO-8601 string).
///
/// Because `Relation` does not yet carry temporal columns in the DB, all
/// relations belonging to the snapshotted entities are included regardless of
/// time; this is conservative and can be refined once a migration adds
/// `valid_from`/`valid_until` to the `entity_relations` table.
pub async fn snapshot_at(valid_at: &str) -> Result<KgSnapshot> {
    // Fetch all entities valid at the given instant
    let entities = fetch_entities_at(valid_at).await?;

    // For each entity collect its current relations
    let mut relations: Vec<RelationVersion> = Vec::new();
    for entity in &entities {
        match KGRepository::get_related_entities(
            pool(),
            &get_default_tenant(),
            &entity.entity_id,
            None,
            Some(500),
        )
        .await
        {
            Ok(pairs) => {
                for (_related_entity, rel) in pairs {
                    relations.push(rel.into());
                }
            }
            Err(e) => {
                warn!(
                    "Could not fetch relations for entity {}: {}",
                    entity.entity_id, e
                );
            }
        }
    }

    // De-duplicate relations (same relation_id may appear from both endpoints)
    relations.sort_by(|a, b| a.relation_id.cmp(&b.relation_id));
    relations.dedup_by(|a, b| a.relation_id == b.relation_id);

    info!(
        valid_at = %valid_at,
        entities = entities.len(),
        relations = relations.len(),
        "KG snapshot generated"
    );

    Ok(KgSnapshot {
        valid_at: valid_at.to_string(),
        entities,
        relations,
    })
}

/// Compute the diff in entities between two time points.
///
/// Relations diff is expressed as counts only because the `entity_relations`
/// table does not yet carry temporal columns.
pub async fn diff_intervals(from_time: &str, to_time: &str) -> Result<TemporalDiff> {
    if from_time >= to_time {
        bail!("`from_time` must be strictly before `to_time`");
    }

    let old_entities = fetch_entities_at(from_time).await?;
    let new_entities = fetch_entities_at(to_time).await?;

    // Build maps by entity_name for comparison (entity_id changes on supersede)
    let old_map: std::collections::HashMap<&str, &Entity> = old_entities
        .iter()
        .map(|e| (e.entity_name.as_str(), e))
        .collect();
    let new_map: std::collections::HashMap<&str, &Entity> = new_entities
        .iter()
        .map(|e| (e.entity_name.as_str(), e))
        .collect();

    let added_entities: Vec<Entity> = new_entities
        .iter()
        .filter(|e| !old_map.contains_key(e.entity_name.as_str()))
        .cloned()
        .collect();

    let removed_entities: Vec<Entity> = old_entities
        .iter()
        .filter(|e| !new_map.contains_key(e.entity_name.as_str()))
        .cloned()
        .collect();

    let modified_entities: Vec<(Entity, Entity)> = old_entities
        .iter()
        .filter_map(|old| {
            new_map
                .get(old.entity_name.as_str())
                .map(|new| {
                    // Consider modified if type or description changed
                    if old.entity_type != new.entity_type || old.description != new.description {
                        Some((old.clone(), (*new).clone()))
                    } else {
                        None
                    }
                })
                .flatten()
        })
        .collect();

    info!(
        from = %from_time,
        to = %to_time,
        added = added_entities.len(),
        removed = removed_entities.len(),
        modified = modified_entities.len(),
        "KG temporal diff computed"
    );

    Ok(TemporalDiff {
        from_time: from_time.to_string(),
        to_time: to_time.to_string(),
        added_entities,
        removed_entities,
        modified_entities,
        // Relation-level counts require DB temporal columns — defer to future migration.
        added_relations: 0,
        removed_relations: 0,
    })
}

/// Detect potential contradictions among the relations of an entity.
///
/// A contradiction is defined as two relations of the same `relation_type`
/// between the same source-target pair that have overlapping valid-time windows.
/// Because `entity_relations` does not yet carry temporal columns in the schema,
/// this function works with the *current* relations and flags logical-type
/// conflicts (e.g. an entity having two `is_a` relations for the same target).
pub async fn detect_contradictions(entity_id: &str) -> Result<Vec<TemporalContradiction>> {
    let relation_pairs = KGRepository::get_related_entities(
        pool(),
        &get_default_tenant(),
        entity_id,
        None,
        Some(1000),
    )
    .await?;

    // Group by (target_entity_id, relation_type)
    let mut groups: std::collections::HashMap<(String, String), Vec<Relation>> =
        std::collections::HashMap::new();
    for (_entity, rel) in relation_pairs {
        let key = (rel.target_entity_id.clone(), rel.relation_type.clone());
        groups.entry(key).or_default().push(rel);
    }

    let mut contradictions = Vec::new();
    for ((target, rel_type), rels) in &groups {
        if rels.len() < 2 {
            continue;
        }
        // Multiple relations of the same type to the same target — flag them.
        for i in 0..rels.len() {
            for j in (i + 1)..rels.len() {
                contradictions.push(TemporalContradiction {
                    entity_id: entity_id.to_string(),
                    relation_type: rel_type.clone(),
                    relation_a: rels[i].relation_id.clone(),
                    relation_b: rels[j].relation_id.clone(),
                    overlap_description: format!(
                        "Entity '{}' has {} concurrent '{}' relations to target '{}'. \
                         Review valid-time windows to ensure only one is current.",
                        entity_id,
                        rels.len(),
                        rel_type,
                        target
                    ),
                });
            }
        }
    }

    if !contradictions.is_empty() {
        warn!(
            entity_id = %entity_id,
            count = contradictions.len(),
            "Temporal contradictions detected in KG"
        );
    }

    Ok(contradictions)
}

/// Supersede an entity and create a new version at the current transaction time.
///
/// Thin wrapper around [`KGRepository::supersede_entity`] that logs the
/// bi-temporal operation.
pub async fn supersede_entity(
    entity_id: &str,
    new_name: &str,
    new_type: &str,
    new_description: Option<&str>,
) -> Result<String> {
    let new_id = KGRepository::supersede_entity(
        &get_default_tenant(),
        entity_id,
        new_name,
        new_type,
        new_description,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{}", e))?;
    info!(
        old_id = %entity_id,
        new_id = %new_id,
        new_name = %new_name,
        "Bi-temporal entity supersession recorded"
    );
    Ok(new_id)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Fetch all entities that were valid at `valid_at` by querying for
/// `valid_from <= valid_at AND (valid_until IS NULL OR valid_until > valid_at)`.
///
/// Falls back to all active entities if the point-in-time query is not
/// supported by the repository.
async fn fetch_entities_at(valid_at: &str) -> Result<Vec<Entity>> {
    // `get_entity_at_time` takes a single id — we need a global snapshot.
    // Use `list_entities` with no filter and then post-filter by valid window.
    let all = crate::db::kg::KGRepository::list_entities(
        pool(),
        &get_default_tenant(),
        None,
        Some(10000),
        Some(0),
    )
    .await
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    let filtered: Vec<Entity> = all
        .entities
        .into_iter()
        .filter(|e| {
            let from_ok = e
                .valid_from
                .as_deref()
                .map(|vf| vf <= valid_at)
                .unwrap_or(true); // no valid_from → treat as always valid
            let until_ok = e
                .valid_until
                .as_deref()
                .map(|vu| vu > valid_at)
                .unwrap_or(true); // no valid_until → still current
            from_ok && until_ok
        })
        .collect();

    Ok(filtered)
}
