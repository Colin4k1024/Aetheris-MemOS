-- Memory Distillation Pipeline tables
-- Implements a multi-layer distillation system:
--   L1: Structured memory atoms (persona, episodic, instruction)
--   L2: Consolidated scene documents
--   L3: Persona profiles
-- Plus skill extraction, agent-asset binding, and async job tracking.

-- L1: Structured memory atoms
CREATE TABLE IF NOT EXISTS distillation_atoms (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    atom_type TEXT NOT NULL CHECK (atom_type IN ('persona', 'episodic', 'instruction')),
    scene_name TEXT NOT NULL,
    content TEXT NOT NULL,
    priority REAL NOT NULL DEFAULT 0.5,
    source_session_id TEXT NOT NULL,
    source_message_ids JSONB NOT NULL DEFAULT '[]',
    metadata JSONB NOT NULL DEFAULT '{}',
    embedding_model TEXT,
    embedding_dimension INTEGER,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    superseded_by TEXT REFERENCES distillation_atoms(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_atoms_tenant_user ON distillation_atoms(tenant_id, user_id, agent_id);
CREATE INDEX IF NOT EXISTS idx_atoms_scene ON distillation_atoms(tenant_id, user_id, scene_name);
CREATE INDEX IF NOT EXISTS idx_atoms_type ON distillation_atoms(atom_type);
CREATE INDEX IF NOT EXISTS idx_atoms_active ON distillation_atoms(is_active) WHERE is_active = TRUE;

-- L2: Consolidated scene documents
CREATE TABLE IF NOT EXISTS distillation_scenes (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    scene_name TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    atom_ids JSONB NOT NULL DEFAULT '[]',
    version INTEGER NOT NULL DEFAULT 1,
    token_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, user_id, agent_id, scene_name)
);

-- L3: Persona profiles
CREATE TABLE IF NOT EXISTS distillation_personas (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    profile_content TEXT NOT NULL,
    scene_ids JSONB NOT NULL DEFAULT '[]',
    version INTEGER NOT NULL DEFAULT 1,
    token_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, user_id, agent_id)
);

-- Extracted reusable skill assets
CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    trigger_conditions JSONB NOT NULL DEFAULT '[]',
    execution_steps JSONB NOT NULL DEFAULT '[]',
    validation_rules JSONB NOT NULL DEFAULT '[]',
    source_session_ids JSONB NOT NULL DEFAULT '[]',
    owner_agent_id TEXT NOT NULL,
    visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private', 'team', 'public')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'deprecated')),
    embedding_model TEXT,
    embedding_dimension INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_skills_tenant ON skills(tenant_id);
CREATE INDEX IF NOT EXISTS idx_skills_agent ON skills(tenant_id, owner_agent_id);
CREATE INDEX IF NOT EXISTS idx_skills_status ON skills(status) WHERE status = 'active';
CREATE UNIQUE INDEX IF NOT EXISTS idx_skills_name_version ON skills(tenant_id, name, version);

-- Agent-asset binding table
CREATE TABLE IF NOT EXISTS agent_equipment (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    asset_type TEXT NOT NULL CHECK (asset_type IN ('skill', 'l1_memory', 'l2_scene', 'l3_persona')),
    asset_id TEXT NOT NULL,
    binding_type TEXT NOT NULL DEFAULT 'fixed' CHECK (binding_type IN ('fixed', 'dynamic', 'conditional')),
    condition JSONB,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(tenant_id, agent_id, asset_type, asset_id)
);

CREATE INDEX IF NOT EXISTS idx_equip_agent ON agent_equipment(tenant_id, agent_id);

-- Async distillation pipeline job tracking
CREATE TABLE IF NOT EXISTS distillation_jobs (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    job_type TEXT NOT NULL CHECK (job_type IN ('l0_to_l1', 'l1_to_l2', 'l2_to_l3', 'skill_extract')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    error_message TEXT,
    atoms_created INTEGER DEFAULT 0,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_jobs_status ON distillation_jobs(status) WHERE status IN ('pending', 'running');
CREATE INDEX IF NOT EXISTS idx_jobs_tenant ON distillation_jobs(tenant_id, session_id);
