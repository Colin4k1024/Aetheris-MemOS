-- Memory Distillation Pipeline Tables
-- L1: Memory Atoms, L2: Scene Blocks, L3: Personas

-- L1: Structured memory atoms extracted from conversations
CREATE TABLE IF NOT EXISTS memory_atoms (
    id TEXT PRIMARY KEY,
    atom_type TEXT NOT NULL CHECK (atom_type IN ('persona', 'episodic', 'instruction')),
    content TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 50,
    scene_name TEXT NOT NULL DEFAULT '',
    source_message_ids TEXT NOT NULL DEFAULT '[]',
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL DEFAULT '',
    tenant_id TEXT NOT NULL,
    metadata TEXT NOT NULL DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_memory_atoms_user_tenant ON memory_atoms(tenant_id, user_id);
CREATE INDEX IF NOT EXISTS idx_memory_atoms_type ON memory_atoms(atom_type);
CREATE INDEX IF NOT EXISTS idx_memory_atoms_scene ON memory_atoms(scene_name);
CREATE INDEX IF NOT EXISTS idx_memory_atoms_session ON memory_atoms(session_id);
CREATE INDEX IF NOT EXISTS idx_memory_atoms_priority ON memory_atoms(priority DESC);

-- L2: Scene blocks — narrative consolidation of L1 atoms
CREATE TABLE IF NOT EXISTS scene_blocks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL,
    heat REAL NOT NULL DEFAULT 0.0,
    atom_ids TEXT NOT NULL DEFAULT '[]',
    user_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_scene_blocks_user_tenant ON scene_blocks(tenant_id, user_id);
CREATE INDEX IF NOT EXISTS idx_scene_blocks_heat ON scene_blocks(heat DESC);

-- L3: User/Agent persona — synthesized profile document
CREATE TABLE IF NOT EXISTS personas (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL DEFAULT '',
    tenant_id TEXT NOT NULL,
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    generated_from_scenes TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(tenant_id, user_id, agent_id)
);

-- Skills table for Phase 3
CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'archived')),
    trigger_conditions TEXT NOT NULL DEFAULT '[]',
    execution_steps TEXT NOT NULL DEFAULT '[]',
    validation_rules TEXT NOT NULL DEFAULT '[]',
    owner_user_id TEXT NOT NULL,
    owner_agent_id TEXT NOT NULL DEFAULT '',
    tenant_id TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'team', 'restricted')),
    tags TEXT NOT NULL DEFAULT '[]',
    usage_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_skills_tenant_owner ON skills(tenant_id, owner_user_id);
CREATE INDEX IF NOT EXISTS idx_skills_status ON skills(status);
CREATE INDEX IF NOT EXISTS idx_skills_visibility ON skills(visibility);

-- Skill versions history
CREATE TABLE IF NOT EXISTS skill_versions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(skill_id, version)
);

-- Agent equipment (memory loadout bindings)
CREATE TABLE IF NOT EXISTS agent_equipment (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    equipped_skills TEXT NOT NULL DEFAULT '[]',
    equipped_scenes TEXT NOT NULL DEFAULT '[]',
    persona_override TEXT,
    memory_filters TEXT NOT NULL DEFAULT '{}',
    recall_strategy TEXT NOT NULL DEFAULT 'hybrid',
    max_recall_tokens INTEGER NOT NULL DEFAULT 2000,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(tenant_id, agent_id)
);
