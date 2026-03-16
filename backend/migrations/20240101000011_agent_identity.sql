-- Agent Identity and Self-Model Tables

-- 1. agent_identities - Core Agent Identity Table
CREATE TABLE IF NOT EXISTS agent_identities (
    agent_id TEXT PRIMARY KEY,
    agent_name TEXT NOT NULL,
    agent_type TEXT NOT NULL DEFAULT 'general',
    version TEXT NOT NULL DEFAULT '1.0.0',

    -- Core Capabilities (JSON for flexibility)
    capabilities JSONB NOT NULL DEFAULT '{}',

    -- Self-Description
    description TEXT,
    personality_traits JSONB,
    system_prompt TEXT,
    core_directives JSONB DEFAULT '[]',

    -- Memory configuration
    memory_config JSONB DEFAULT '{}',

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,

    -- Status
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'suspended')),

    -- Metadata
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_agent_identities_type ON agent_identities(agent_type);
CREATE INDEX IF NOT EXISTS idx_agent_identities_status ON agent_identities(status);

-- 2. agent_capabilities - Agent Capabilities Tracking
CREATE TABLE IF NOT EXISTS agent_capabilities (
    capability_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agent_identities(agent_id) ON DELETE CASCADE,

    -- Capability definition
    capability_name TEXT NOT NULL,
    capability_type TEXT NOT NULL,
    description TEXT,

    -- Implementation
    implementation_type TEXT,
    implementation_ref TEXT,

    -- Metrics
    success_rate REAL DEFAULT 0.0,
    avg_latency_ms INTEGER,
    times_invoked INTEGER DEFAULT 0,

    -- Limits
    max_tokens INTEGER,
    timeout_ms INTEGER,

    -- Status
    enabled BOOLEAN DEFAULT true,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_agent_capabilities_agent ON agent_capabilities(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_capabilities_type ON agent_capabilities(capability_type);

-- 3. agent_behavior_profiles - Learned Behavioral Patterns
CREATE TABLE IF NOT EXISTS agent_behavior_profiles (
    profile_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agent_identities(agent_id) ON DELETE CASCADE,

    -- Behavior type
    behavior_type TEXT NOT NULL,

    -- Pattern definition
    pattern_description TEXT NOT NULL,
    pattern_embedding TEXT,

    -- Usage statistics
    times_applied INTEGER DEFAULT 0,
    success_rate REAL DEFAULT 0.0,
    avg_outcome_score REAL DEFAULT 0.0,

    -- Contexts where this behavior works well
    effective_contexts JSONB DEFAULT '[]',

    -- Confidence in this behavior pattern
    confidence REAL DEFAULT 0.5,

    -- Status
    status TEXT DEFAULT 'active',

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    last_used_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_behavior_profiles_agent ON agent_behavior_profiles(agent_id);
CREATE INDEX IF NOT EXISTS idx_behavior_profiles_type ON agent_behavior_profiles(behavior_type);

-- 4. agent_episodes - Experience Records for Self-Reflection
CREATE TABLE IF NOT EXISTS agent_episodes (
    episode_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agent_identities(agent_id) ON DELETE CASCADE,

    -- Episode context
    episode_type TEXT NOT NULL,
    start_time TIMESTAMPTZ NOT NULL,
    end_time TIMESTAMPTZ,

    -- What happened
    situation TEXT NOT NULL,
    actions_taken JSONB NOT NULL DEFAULT '[]',
    outcome TEXT NOT NULL,

    -- Outcome metrics
    outcome_score REAL,
    success BOOLEAN,

    -- Self-assessment
    what_went_well TEXT,
    what_could_improve TEXT,
    lessons_learned TEXT,

    -- Memory links
    related_episode_ids TEXT[] DEFAULT '{}',
    relevant_knowledge_ids TEXT[] DEFAULT '{}',

    -- Reflection metadata
    reflection_level INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_agent_episodes_agent ON agent_episodes(agent_id);
CREATE INDEX IF NOT EXISTS idx_agent_episodes_type ON agent_episodes(episode_type);
CREATE INDEX IF NOT EXISTS idx_agent_episodes_time ON agent_episodes(start_time);
CREATE INDEX IF NOT EXISTS idx_agent_episodes_outcome ON agent_episodes(outcome_score);

-- 5. agent_self_models - Agent's Self-Model (Meta-Memory)
CREATE TABLE IF NOT EXISTS agent_self_models (
    model_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL UNIQUE REFERENCES agent_identities(agent_id) ON DELETE CASCADE,

    -- Identity beliefs
    identity_beliefs JSONB DEFAULT '[]',

    -- Competencies
    strengths JSONB DEFAULT '[]',
    weaknesses JSONB DEFAULT '[]',
    learned_skills JSONB DEFAULT '[]',

    -- Preferences
    preferences JSONB DEFAULT '{}',

    -- Relationships with other agents
    relationships JSONB DEFAULT '[]',

    -- Computed personality traits
    computed_traits JSONB DEFAULT '{}',

    -- Confidence scores
    confidence_score REAL DEFAULT 0.5,
    consistency_score REAL DEFAULT 1.0,

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_agent_self_models_agent ON agent_self_models(agent_id);
