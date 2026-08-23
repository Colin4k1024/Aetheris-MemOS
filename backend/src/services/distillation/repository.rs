use anyhow::Result;
use sqlx::{Pool, Sqlite};
use tracing::{error, info};
use ulid::Ulid;

use super::types::*;

pub struct DistillationRepository;

impl DistillationRepository {
    // ─── Memory Atoms (L1) ───

    pub async fn insert_atom(pool: &Pool<Sqlite>, atom: &MemoryAtom) -> Result<()> {
        let source_ids_json = serde_json::to_value(&atom.source_message_ids)?;
        let atom_type_str = atom.atom_type.as_str();

        sqlx::query(
            r#"INSERT INTO memory_atoms (id, atom_type, content, priority, scene_name, source_message_ids, session_id, user_id, agent_id, tenant_id, metadata, version, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&atom.id)
        .bind(atom_type_str)
        .bind(&atom.content)
        .bind(atom.priority)
        .bind(&atom.scene_name)
        .bind(&source_ids_json)
        .bind(&atom.session_id)
        .bind(&atom.user_id)
        .bind(&atom.agent_id)
        .bind(&atom.tenant_id)
        .bind(&atom.metadata)
        .bind(atom.version)
        .bind(&atom.created_at)
        .bind(&atom.updated_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_atoms_by_user(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
        limit: i64,
    ) -> Result<Vec<MemoryAtom>> {
        let rows: Vec<MemoryAtomRow> = sqlx::query_as(
            r#"SELECT * FROM memory_atoms WHERE tenant_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT ?"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(MemoryAtom::from).collect())
    }

    pub async fn get_atoms_by_session(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        session_id: &str,
    ) -> Result<Vec<MemoryAtom>> {
        let rows: Vec<MemoryAtomRow> = sqlx::query_as(
            r#"SELECT * FROM memory_atoms WHERE tenant_id = ? AND session_id = ? ORDER BY created_at ASC"#,
        )
        .bind(tenant_id)
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(MemoryAtom::from).collect())
    }

    pub async fn get_atoms_by_type(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
        atom_type: MemoryAtomType,
        limit: i64,
    ) -> Result<Vec<MemoryAtom>> {
        let rows: Vec<MemoryAtomRow> = sqlx::query_as(
            r#"SELECT * FROM memory_atoms WHERE tenant_id = ? AND user_id = ? AND atom_type = ? ORDER BY priority DESC, created_at DESC LIMIT ?"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(atom_type.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(MemoryAtom::from).collect())
    }

    pub async fn update_atom_version(
        pool: &Pool<Sqlite>,
        atom_id: &str,
        new_content: &str,
        new_version: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"UPDATE memory_atoms SET content = ?, version = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"#,
        )
        .bind(new_content)
        .bind(new_version)
        .bind(atom_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn count_atoms_since(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
        since: &str,
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM memory_atoms WHERE tenant_id = ? AND user_id = ? AND created_at > ?"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(since)
        .fetch_one(pool)
        .await?;

        Ok(row.0)
    }

    pub async fn search_atoms_by_content(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
        query: &str,
        limit: i64,
    ) -> Result<Vec<MemoryAtom>> {
        let pattern = format!("%{}%", query);
        let rows: Vec<MemoryAtomRow> = sqlx::query_as(
            r#"SELECT * FROM memory_atoms WHERE tenant_id = ? AND user_id = ? AND content LIKE ? ORDER BY priority DESC LIMIT ?"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(MemoryAtom::from).collect())
    }

    // ─── Scene Blocks (L2) ───

    pub async fn upsert_scene(pool: &Pool<Sqlite>, scene: &SceneBlock) -> Result<()> {
        let atom_ids_json = serde_json::to_value(&scene.atom_ids)?;

        sqlx::query(
            r#"INSERT INTO scene_blocks (id, name, summary, content, heat, atom_ids, user_id, tenant_id, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 summary = excluded.summary,
                 content = excluded.content,
                 heat = excluded.heat,
                 atom_ids = excluded.atom_ids,
                 updated_at = excluded.updated_at"#,
        )
        .bind(&scene.id)
        .bind(&scene.name)
        .bind(&scene.summary)
        .bind(&scene.content)
        .bind(scene.heat)
        .bind(&atom_ids_json)
        .bind(&scene.user_id)
        .bind(&scene.tenant_id)
        .bind(&scene.created_at)
        .bind(&scene.updated_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_scenes_by_user(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<Vec<SceneBlock>> {
        let rows: Vec<SceneBlockRow> = sqlx::query_as(
            r#"SELECT * FROM scene_blocks WHERE tenant_id = ? AND user_id = ? ORDER BY heat DESC, updated_at DESC"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(SceneBlock::from).collect())
    }

    pub async fn increment_scene_heat(pool: &Pool<Sqlite>, scene_id: &str) -> Result<()> {
        sqlx::query(
            r#"UPDATE scene_blocks SET heat = heat + 1.0, updated_at = CURRENT_TIMESTAMP WHERE id = ?"#,
        )
        .bind(scene_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn count_scenes(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM scene_blocks WHERE tenant_id = ? AND user_id = ?"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;

        Ok(row.0)
    }

    // ─── Persona (L3) ───

    pub async fn upsert_persona(pool: &Pool<Sqlite>, persona: &Persona) -> Result<()> {
        let scenes_json = serde_json::to_value(&persona.generated_from_scenes)?;
        let agent_id = persona.agent_id.as_deref().unwrap_or("");

        sqlx::query(
            r#"INSERT INTO personas (id, user_id, agent_id, tenant_id, content, version, generated_from_scenes, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(tenant_id, user_id, agent_id) DO UPDATE SET
                 content = excluded.content,
                 version = excluded.version,
                 generated_from_scenes = excluded.generated_from_scenes,
                 updated_at = excluded.updated_at"#,
        )
        .bind(&persona.id)
        .bind(&persona.user_id)
        .bind(agent_id)
        .bind(&persona.tenant_id)
        .bind(&persona.content)
        .bind(persona.version)
        .bind(&scenes_json)
        .bind(&persona.created_at)
        .bind(&persona.updated_at)
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn get_persona(
        pool: &Pool<Sqlite>,
        tenant_id: &str,
        user_id: &str,
        agent_id: Option<&str>,
    ) -> Result<Option<Persona>> {
        let aid = agent_id.unwrap_or("");
        let row: Option<PersonaRow> = sqlx::query_as(
            r#"SELECT * FROM personas WHERE tenant_id = ? AND user_id = ? AND agent_id = ?"#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(aid)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(Persona::from))
    }

    pub fn generate_id() -> String {
        Ulid::new().to_string()
    }
}
