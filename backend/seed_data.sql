-- 初始化数据库数据
-- Memory Configurations - 默认配置 (使用 ON CONFLICT DO NOTHING 避免重复)

-- 1. 默认配置 (Default)
INSERT INTO memory_configurations (
    config_id, user_id, agent_id, config_name, config_type,
    stm_enabled, stm_max_length, stm_retention_hours,
    ltm_enabled, ltm_max_entries, ltm_quality_threshold,
    kg_enabled, kg_max_entities, kg_confidence_threshold,
    mm_enabled, mm_max_entries, mm_modality_types,
    max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
    status
) VALUES (
    'config_default_001', 'admin', 'default_agent', 'Default Configuration', 'default',
    1, 4096, 24,
    1, 10000, 0.50,
    0, 1000, 0.70,
    0, 1000, 'text,image',
    2000, 1024, 80,
    'active'
) ON CONFLICT (config_id) DO NOTHING;

-- 2. 优化配置 (Optimized for Performance)
INSERT INTO memory_configurations (
    config_id, user_id, agent_id, config_name, config_type,
    stm_enabled, stm_max_length, stm_retention_hours,
    ltm_enabled, ltm_max_entries, ltm_quality_threshold,
    kg_enabled, kg_max_entities, kg_confidence_threshold,
    mm_enabled, mm_max_entries, mm_modality_types,
    max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
    status
) VALUES (
    'config_optimized_001', 'admin', 'default_agent', 'Performance Optimized', 'optimized',
    1, 8192, 48,
    1, 50000, 0.70,
    1, 5000, 0.80,
    1, 5000, 'text,image,audio',
    1000, 2048, 90,
    'active'
) ON CONFLICT (config_id) DO NOTHING;

-- 3. 轻量配置 (Custom - Lightweight)
INSERT INTO memory_configurations (
    config_id, user_id, agent_id, config_name, config_type,
    stm_enabled, stm_max_length, stm_retention_hours,
    ltm_enabled, ltm_max_entries, ltm_quality_threshold,
    kg_enabled, kg_max_entities, kg_confidence_threshold,
    mm_enabled, mm_max_entries, mm_modality_types,
    max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
    status
) VALUES (
    'config_light_001', 'admin', 'default_agent', 'Lightweight Configuration', 'custom',
    1, 2048, 12,
    1, 1000, 0.30,
    0, 500, 0.60,
    0, 100, 'text',
    3000, 512, 50,
    'active'
) ON CONFLICT (config_id) DO NOTHING;

-- 4. 知识图谱启用配置
INSERT INTO memory_configurations (
    config_id, user_id, agent_id, config_name, config_type,
    stm_enabled, stm_max_length, stm_retention_hours,
    ltm_enabled, ltm_max_entries, ltm_quality_threshold,
    kg_enabled, kg_max_entities, kg_confidence_threshold,
    mm_enabled, mm_max_entries, mm_modality_types,
    max_response_time_ms, max_memory_usage_mb, max_cpu_usage_percent,
    status
) VALUES (
    'config_kg_001', 'admin', 'default_agent', 'Knowledge Graph Enabled', 'optimized',
    1, 4096, 24,
    1, 20000, 0.60,
    1, 10000, 0.75,
    1, 2000, 'text,image',
    1500, 1536, 70,
    'active'
) ON CONFLICT (config_id) DO NOTHING;

-- Performance Metrics - 基线数据
INSERT INTO performance_metrics (
    metric_id, session_id, config_id, date_hour,
    response_time_ms, memory_usage_mb, cpu_usage_percent,
    stm_usage_count, ltm_usage_count, kg_usage_count, mm_usage_count,
    accuracy_score, coherence_score, user_satisfaction, error_count
) VALUES
('metric_001', 'session_001', 'config_default_001', '2026-03-06-10',
 1850, 512, 45, 120, 45, 0, 0, 0.85, 0.82, 0.78, 2),
('metric_002', 'session_002', 'config_default_001', '2026-03-06-11',
 1920, 548, 52, 135, 52, 0, 0, 0.83, 0.80, 0.75, 3),
('metric_003', 'session_003', 'config_optimized_001', '2026-03-06-10',
 950, 1024, 78, 280, 156, 45, 12, 0.92, 0.89, 0.88, 1),
('metric_004', 'session_004', 'config_optimized_001', '2026-03-06-11',
 1020, 1152, 82, 310, 178, 52, 15, 0.90, 0.87, 0.85, 2),
('metric_005', 'session_005', 'config_light_001', '2026-03-06-10',
 2800, 256, 25, 45, 12, 0, 0, 0.75, 0.72, 0.70, 1),
('metric_006', 'session_006', 'config_light_001', '2026-03-06-11',
 2950, 280, 28, 52, 15, 0, 0, 0.73, 0.70, 0.68, 2),
('metric_007', 'session_007', 'config_kg_001', '2026-03-06-10',
 1420, 768, 58, 180, 89, 120, 28, 0.88, 0.85, 0.82, 1),
('metric_008', 'session_008', 'config_kg_001', '2026-03-06-11',
 1480, 820, 62, 195, 95, 135, 32, 0.86, 0.83, 0.80, 2)
ON CONFLICT (metric_id) DO NOTHING;

-- Weight History - 权重历史 (使用正确的表结构)
INSERT INTO weight_adjustment_history (
    history_id, task_id, old_weights_json, new_weights_json,
    adjustment_reasons_json, performance_impact, strategy_metadata, timestamp
) VALUES
('weight_001', 'task_default', '{"stm":0.20,"ltm":0.25,"kg":0.20,"mm":0.15}',
 '{"stm":0.20,"ltm":0.25,"kg":0.20,"mm":0.15}',
 '{"stm":"Standard","ltm":"Standard","kg":"Standard","mm":"Disabled"}',
 0.05, '["MarginalBenefit"]', '2026-03-05 10:00:00'),
('weight_002', 'task_default', '{"stm":0.20,"ltm":0.25,"kg":0.20,"mm":0.15}',
 '{"stm":0.22,"ltm":0.28,"kg":0.18,"mm":0.12}',
 '{"stm":"Increased","ltm":"Increased","kg":"Reduced","mm":"Reduced"}',
 0.08, '["MarginalBenefit"]', '2026-03-06 09:00:00'),
('weight_003', 'task_test', '{"stm":0.22,"ltm":0.28,"kg":0.18,"mm":0.12}',
 '{"stm":0.25,"ltm":0.30,"kg":0.20,"mm":0.15}',
 '{"stm":"Further increased","ltm":"Further increased","kg":"Increased","mm":"Enabled"}',
 0.12, '["MarginalBenefit","LinearDecay"]', '2026-03-06 12:00:00');

SELECT 'Data initialized successfully!' as result;
