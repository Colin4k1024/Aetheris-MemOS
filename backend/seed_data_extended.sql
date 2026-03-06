-- 扩展初始化数据 - STM, LTM, KG, Multimodal

-- =====================
-- 1. 短期记忆 (STM) - 会话消息
-- =====================

-- 创建测试会话
INSERT INTO context_sessions (
    session_id, user_id, agent_id, created_at, updated_at, expires_at,
    session_type, context_length, max_context_length, status, priority
) VALUES
('session_test_001', 'admin', 'default_agent', '2026-03-06 10:00:00+00', '2026-03-06 10:30:00+00', '2026-03-07 10:00:00+00',
 'conversation', 2048, 4096, 'active', 8),
('session_test_002', 'admin', 'default_agent', '2026-03-06 11:00:00+00', '2026-03-06 11:30:00+00', '2026-03-07 11:00:00+00',
 'task', 1536, 4096, 'active', 7),
('session_test_003', 'admin', 'default_agent', '2026-03-06 12:00:00+00', '2026-03-06 12:15:00+00', '2026-03-07 12:00:00+00',
 'query', 512, 2048, 'completed', 5)
ON CONFLICT (session_id) DO NOTHING;

-- 添加会话消息
INSERT INTO session_messages (
    message_id, session_id, role, content, created_at, token_count, importance_score
) VALUES
-- Session 1 messages
('msg_001_001', 'session_test_001', 'user', '你好，我想了解一下自适应记忆系统的功能', '2026-03-06 10:00:00+00', 25, 0.7),
('msg_001_002', 'session_test_001', 'assistant', '您好！自适应记忆系统是一个智能化的AI记忆管理系统，它能够根据不同的任务需求自动选择最合适的记忆配置。它支持短期记忆(LTM)、长期记忆(STM)、知识图谱(KG)和多模态记忆(MM)等多种记忆类型。', '2026-03-06 10:00:30+00', 80, 0.8),
('msg_001_003', 'session_test_001', 'user', '那它如何决定使用哪种记忆配置呢？', '2026-03-06 10:01:00+00', 30, 0.9),
('msg_001_004', 'session_test_001', 'assistant', '系统会根据任务特征分析器来分析任务的复杂度、模态类型和推理深度，然后预测器会评估不同配置的性能表现，最后调度器会选择最优的配置方案。整个过程是自动化的。', '2026-03-06 10:01:30+00', 95, 0.85),
('msg_001_005', 'session_test_001', 'user', '听起来很智能！可以举例说明吗？', '2026-03-06 10:02:00+00', 20, 0.6),
('msg_001_006', 'session_test_001', 'assistant', '当然！例如对于简单的问答任务，系统会选择轻量级配置以提高响应速度；对于复杂的推理任务，会启用知识图谱来增强上下文理解；对于需要处理图片、音频的任务，则会启用多模态记忆功能。', '2026-03-06 10:02:30+00', 110, 0.75),

-- Session 2 messages
('msg_002_001', 'session_test_002', 'user', '帮我分析一下这个代码的性能问题', '2026-03-06 11:00:00+00', 35, 0.9),
('msg_002_002', 'session_test_002', 'assistant', '好的，请提供您想要分析的代码片段。我会从算法复杂度、内存使用、CPU消耗等方面进行全面分析。', '2026-03-06 11:00:30+00', 55, 0.8),

-- Session 3 messages
('msg_003_001', 'session_test_003', 'user', '查询今天天气', '2026-03-06 12:00:00+00', 10, 0.5),
('msg_003_002', 'session_test_003', 'assistant', '今天天气晴朗，温度20-28摄氏度，适合外出。', '2026-03-06 12:00:15+00', 30, 0.5)
ON CONFLICT (message_id) DO NOTHING;

-- =====================
-- 2. 长期记忆 (LTM) - 知识条目
-- =====================

INSERT INTO knowledge_entries (
    entry_id, source_id, source_type, title, content, content_type, content_hash,
    embedding_vector, embedding_model, embedding_dimension, category, domain,
    quality_score, relevance_score, confidence_score, access_count, success_count, status
) VALUES
('ltm_001', 'doc_001', 'document', '自适应记忆系统架构',
 'Adaptive Memory System是一个智能化的记忆管理系统，采用多层架构设计。核心组件包括：任务特征分析器(Task Analyzer)负责分析任务特征；性能预测器(Predictor)预测不同配置的性能；调度器(Scheduler)选择最优配置；权重调整器(Weight Adjuster)动态调整系统权重。系统支持四种记忆类型：短期记忆(STM)、长期记忆(LTM)、知识图谱(KG)和多模态记忆(MM)。',
 'text', 'a1b2c3d4e5f6', '[0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8]', 'nomic-embed-text', 768,
 '技术文档', 'AI系统', 0.92, 0.88, 0.95, 15, 12, 'active'),

('ltm_002', 'doc_002', 'document', '记忆类型说明',
 '系统支持四种记忆类型：1)短期记忆(STM)用于存储当前会话的上下文信息，容量有限但访问速度快；2)长期记忆(LTM)用于存储持久化知识，支持向量检索；3)知识图谱(KG)用于存储实体关系，支持复杂推理；4)多模态记忆(MM)用于存储图片、音频等非结构化数据。',
 'text', 'b2c3d4e5f6a7', '[0.2,0.3,0.4,0.5,0.6,0.7,0.8,0.9]', 'nomic-embed-text', 768,
 '技术文档', 'AI系统', 0.88, 0.85, 0.90, 8, 7, 'active'),

('ltm_003', 'user_001', 'user_input', '用户偏好设置',
 '用户喜欢使用英文界面；偏好快速响应；对于技术问题希望获得详细解释；常用功能：代码分析、文档理解、问题解答。',
 'text', 'c3d4e5f6a7b8', '[0.3,0.4,0.5,0.6,0.7,0.8,0.9,1.0]', 'nomic-embed-text', 768,
 '用户偏好', '个性化', 0.95, 0.80, 0.92, 20, 18, 'active'),

('ltm_004', 'api_001', 'api', '系统性能基线',
 '默认配置响应时间：1800ms，成功率：85%；优化配置响应时间：950ms，成功率：92%；轻量配置响应时间：2800ms，成功率：75%。',
 'text', 'd4e5f6a7b8c9', '[0.4,0.5,0.6,0.7,0.8,0.9,1.0,1.1]', 'nomic-embed-text', 768,
 '性能数据', '系统', 0.90, 0.82, 0.88, 12, 10, 'active'),

('ltm_005', 'doc_003', 'document', '权重调整策略',
 '系统使用多种权重调整策略：1)边际收益策略(MarginalBenefit)根据性能提升调整权重；2)线性衰减策略(LinearDecay)随时间降低低优先级权重；3)自适应策略根据任务类型自动调整。',
 'text', 'e5f6a7b8c9d0', '[0.5,0.6,0.7,0.8,0.9,1.0,1.1,1.2]', 'nomic-embed-text', 768,
 '技术文档', 'AI系统', 0.85, 0.78, 0.82, 6, 5, 'active')
ON CONFLICT (entry_id) DO NOTHING;

-- 添加知识关系
INSERT INTO knowledge_relations (
    relation_id, source_entry_id, target_entry_id, relation_type,
    relation_strength, relation_confidence, description
) VALUES
('rel_001', 'ltm_001', 'ltm_002', 'extends', 0.95, 0.92, '架构文档详细说明了各记忆类型'),
('rel_002', 'ltm_002', 'ltm_004', 'supports', 0.88, 0.85, '记忆类型影响系统性能'),
('rel_003', 'ltm_001', 'ltm_005', 'related', 0.82, 0.80, '架构与权重调整策略相关'),
('rel_004', 'ltm_003', 'ltm_001', 'related', 0.75, 0.72, '用户偏好影响系统配置')
ON CONFLICT (relation_id) DO NOTHING;

-- =====================
-- 3. 知识图谱 (KG) - Neo4j实体和关系
-- =====================

-- 注意：Neo4j需要通过API或neo4j cypher添加，这里提供示例CQL
-- 以下是Neo4j中需要创建的实体和关系

/*
-- 创建实体
CREATE (a:Entity {id: 'kg_entity_001', name: '自适应记忆系统', type: '系统', description: '智能化的AI记忆管理系统'})
CREATE (b:Entity {id: 'kg_entity_002', name: '短期记忆', type: '概念', description: '用于存储当前会话上下文'})
CREATE (c:Entity {id: 'kg_entity_003', name: '长期记忆', type: '概念', description: '用于存储持久化知识'})
CREATE (d:Entity {id: 'kg_entity_004', name: '知识图谱', type: '概念', description: '用于存储实体关系'})
CREATE (e:Entity {id: 'kg_entity_005', name: '多模态记忆', type: '概念', description: '用于存储非结构化数据'})
CREATE (f:Entity {id: 'kg_entity_006', name: '任务特征分析器', type: '组件', description: '负责分析任务特征'})
CREATE (g:Entity {id: 'kg_entity_007', name: '性能预测器', type: '组件', description: '预测不同配置性能'})
CREATE (h:Entity {id: 'kg_entity_008', name: '调度器', type: '组件', description: '选择最优配置'})

-- 创建关系
CREATE (a)-[:包含 {weight: 0.9}]->(b)
CREATE (a)-[:包含 {weight: 0.9}]->(c)
CREATE (a)-[:包含 {weight: 0.85}]->(d)
CREATE (a)-[:包含 {weight: 0.85}]->(e)
CREATE (a)-[:使用组件 {weight: 0.95}]->(f)
CREATE (a)-[:使用组件 {weight: 0.95}]->(g)
CREATE (a)-[:使用组件 {weight: 0.95}]->(h)
*/

-- =====================
-- 4. 多模态记忆 (MM)
-- =====================

INSERT INTO multimodal_entries (
    entry_id, session_id, source_id, modality_type, title, description,
    content_metadata, text_content, text_embedding, quality_score, created_at, status
) VALUES
('mm_001', 'session_test_001', 'doc_001', 'text', '自适应记忆系统架构', '系统架构文档内容',
 '{"format": "markdown", "language": "zh-CN", "word_count": 500}', 'Adaptive Memory System是一个智能化的记忆管理系统...',
 '[0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8]', 0.90, '2026-03-06 10:00:00+00', 'active'),

('mm_002', 'session_test_001', 'img_001', 'image', '系统架构图', '系统架构流程图',
 '{"format": "png", "resolution": "1920x1080", "description": "系统架构流程图"}', '系统架构图示：包含分析器、预测器、调度器等组件',
 '[0.2,0.3,0.4,0.5,0.6,0.7,0.8,0.9]', 0.85, '2026-03-06 10:05:00+00', 'active'),

('mm_003', 'session_test_002', 'code_001', 'text', '代码分析结果', '性能分析报告',
 '{"format": "json", "language": "zh-CN", "analysis_type": "performance"}', '代码分析结果：算法复杂度O(n log n)，内存使用合理',
 '[0.3,0.4,0.5,0.6,0.7,0.8,0.9,1.0]', 0.88, '2026-03-06 11:00:00+00', 'active')
ON CONFLICT (entry_id) DO NOTHING;

SELECT 'Extended data initialized successfully!' as result;
