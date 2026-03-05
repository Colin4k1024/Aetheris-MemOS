-- =============================================
-- 知识图谱测试数据 (Knowledge Graph Test Data)
-- =============================================

-- 1. 实体 - 人物类
INSERT INTO entities (entity_id, entity_name, entity_type, description, aliases, embedding_vector, embedding_model, embedding_dimension, confidence_score, popularity_score, relation_count, mention_count, status)
VALUES
    ('entity_001', '埃隆·马斯克', '人物', '企业家、工程师，SpaceX和Tesla创始人', '["埃隆","马斯克","Elon Musk"]', '[0.1, 0.2, 0.3, 0.4, 0.5]', 'nomic-embed-text', 768, 0.95, 0.92, 15, 25, 'active'),
    ('entity_002', '史蒂夫·乔布斯', '人物', '苹果公司联合创始人', '["乔布斯","Steve Jobs","史蒂夫"]', '[0.15, 0.25, 0.35, 0.45, 0.55]', 'nomic-embed-text', 768, 0.98, 0.90, 12, 20, 'active'),
    ('entity_003', '比尔·盖茨', '人物', '微软公司联合创始人', '["比尔","盖茨","Bill Gates"]', '[0.2, 0.3, 0.4, 0.5, 0.6]', 'nomic-embed-text', 768, 0.95, 0.88, 10, 18, 'active'),
    ('entity_004', '杰夫·贝索斯', '人物', '亚马逊公司创始人', '["贝索斯","Jeff Bezos","杰夫"]', '[0.25, 0.35, 0.45, 0.55, 0.65]', 'nomic-embed-text', 768, 0.88, 0.82, 8, 12, 'active');

-- 2. 实体 - 公司类
INSERT INTO entities (entity_id, entity_name, entity_type, description, aliases, embedding_vector, embedding_model, embedding_dimension, confidence_score, popularity_score, relation_count, mention_count, status)
VALUES
    ('entity_010', '特斯拉', '公司', '电动汽车和清洁能源公司', '["Tesla","TSLA"]', '[0.3, 0.4, 0.5, 0.6, 0.7]', 'nomic-embed-text', 768, 0.95, 0.95, 20, 35, 'active'),
    ('entity_011', '苹果公司', '公司', '消费电子和软件公司', '["Apple","AAPL","苹果"]', '[0.35, 0.45, 0.55, 0.65, 0.75]', 'nomic-embed-text', 768, 0.98, 0.94, 18, 30, 'active'),
    ('entity_012', '微软', '公司', '软件和云计算公司', '["Microsoft","MSFT","微软公司"]', '[0.4, 0.5, 0.6, 0.7, 0.8]', 'nomic-embed-text', 768, 0.96, 0.92, 16, 28, 'active'),
    ('entity_013', '亚马逊', '公司', '电商和云计算公司', '["Amazon","AMZN","亚马逊公司"]', '[0.45, 0.55, 0.65, 0.75, 0.85]', 'nomic-embed-text', 768, 0.92, 0.90, 14, 24, 'active'),
    ('entity_014', 'SpaceX', '公司', '航天航空公司', '["Space X","太空探索技术公司"]', '[0.5, 0.6, 0.7, 0.8, 0.9]', 'nomic-embed-text', 768, 0.90, 0.88, 12, 22, 'active'),
    ('entity_015', 'OpenAI', '公司', '人工智能研究公司', '["Open AI","开放人工智能"]', '[0.55, 0.65, 0.75, 0.85, 0.95]', 'nomic-embed-text', 768, 0.92, 0.90, 10, 20, 'active');

-- 3. 实体 - 技术类
INSERT INTO entities (entity_id, entity_name, entity_type, description, aliases, embedding_vector, embedding_model, embedding_dimension, confidence_score, popularity_score, relation_count, mention_count, status)
VALUES
    ('entity_020', '人工智能', '技术', '计算机科学的一个分支', '["AI","Artificial Intelligence","机器智能"]', '[0.6, 0.65, 0.7, 0.75, 0.8]', 'nomic-embed-text', 768, 0.98, 0.96, 25, 45, 'active'),
    ('entity_021', '机器学习', '技术', '人工智能的子领域', '["ML","Machine Learning"]', '[0.62, 0.67, 0.72, 0.77, 0.82]', 'nomic-embed-text', 768, 0.96, 0.94, 22, 40, 'active'),
    ('entity_022', '深度学习', '技术', '机器学习的子领域，使用神经网络', '["DL","Deep Learning","神经网络"]', '[0.64, 0.69, 0.74, 0.79, 0.84]', 'nomic-embed-text', 768, 0.94, 0.92, 20, 35, 'active'),
    ('entity_023', '区块链', '技术', '分布式账本技术', '["Blockchain","分布式账本","块链"]', '[0.66, 0.71, 0.76, 0.81, 0.86]', 'nomic-embed-text', 768, 0.88, 0.85, 15, 25, 'active'),
    ('entity_024', '量子计算', '技术', '使用量子力学原理的计算', '["Quantum Computing","量子计算机"]', '[0.68, 0.73, 0.78, 0.83, 0.88]', 'nomic-embed-text', 768, 0.85, 0.80, 12, 20, 'active');

-- 4. 实体 - 产品类
INSERT INTO entities (entity_id, entity_name, entity_type, description, aliases, embedding_vector, embedding_model, embedding_dimension, confidence_score, popularity_score, relation_count, mention_count, status)
VALUES
    ('entity_030', 'iPhone', '产品', '苹果公司的智能手机', '["苹果手机","iPhone手机"]', '[0.7, 0.75, 0.8, 0.85, 0.9]', 'nomic-embed-text', 768, 0.96, 0.95, 15, 28, 'active'),
    ('entity_031', 'Model S', '产品', '特斯拉生产的电动汽车', '["特斯拉Model S","Tesla Model S"]', '[0.72, 0.77, 0.82, 0.87, 0.92]', 'nomic-embed-text', 768, 0.90, 0.88, 10, 18, 'active'),
    ('entity_032', 'ChatGPT', '产品', 'OpenAI的聊天AI产品', '["Chat GPT","GPT聊天"]', '[0.74, 0.79, 0.84, 0.89, 0.94]', 'nomic-embed-text', 768, 0.94, 0.92, 12, 22, 'active'),
    ('entity_033', 'Windows', '产品', '微软的操作系统', '["微软视窗","Win"]', '[0.76, 0.81, 0.86, 0.91, 0.96]', 'nomic-embed-text', 768, 0.98, 0.96, 18, 32, 'active');

-- 5. 实体 - 概念类
INSERT INTO entities (entity_id, entity_name, entity_type, description, aliases, embedding_vector, embedding_model, embedding_dimension, confidence_score, popularity_score, relation_count, mention_count, status)
VALUES
    ('entity_040', '自动驾驶', '概念', '车辆自主驾驶技术', '["无人驾驶","Autonomous Driving"]', '[0.8, 0.82, 0.84, 0.86, 0.88]', 'nomic-embed-text', 768, 0.88, 0.85, 14, 24, 'active'),
    ('entity_041', '云计算', '概念', '通过互联网提供计算资源', '["Cloud Computing","云服务"]', '[0.82, 0.84, 0.86, 0.88, 0.9]', 'nomic-embed-text', 768, 0.92, 0.90, 16, 28, 'active'),
    ('entity_042', '物联网', '概念', '物理设备网络', '["IoT","Internet of Things"]', '[0.84, 0.86, 0.88, 0.9, 0.92]', 'nomic-embed-text', 768, 0.86, 0.82, 12, 20, 'active'),
    ('entity_043', '大语言模型', '概念', '大型语言模型', '["LLM","Large Language Model"]', '[0.86, 0.88, 0.9, 0.92, 0.94]', 'nomic-embed-text', 768, 0.90, 0.88, 18, 30, 'active');

-- 关系数据
INSERT INTO relations (relation_id, source_entity_id, target_entity_id, relation_type, relation_name, description, weight, confidence, usage_count, success_count, status)
VALUES
    -- 人物与公司的关系
    ('rel_001', 'entity_001', 'entity_010', 'founded', '创立', '埃隆·马斯克创立了特斯拉', 0.98, 0.98, 25, 24, 'active'),
    ('rel_002', 'entity_001', 'entity_014', 'founded', '创立', '埃隆·马斯克创立了SpaceX', 0.98, 0.98, 22, 21, 'active'),
    ('rel_003', 'entity_002', 'entity_011', 'founded', '联合创立', '史蒂夫·乔布斯联合创立了苹果', 0.99, 0.99, 30, 29, 'active'),
    ('rel_004', 'entity_003', 'entity_012', 'founded', '联合创立', '比尔·盖茨联合创立了微软', 0.99, 0.99, 28, 27, 'active'),
    ('rel_005', 'entity_004', 'entity_013', 'founded', '创立', '杰夫·贝索斯创立了亚马逊', 0.98, 0.98, 20, 19, 'active'),

    -- 公司之间的关系
    ('rel_010', 'entity_010', 'entity_014', 'competes_with', '竞争', '特斯拉与SpaceX在航天领域有合作关系', 0.60, 0.65, 15, 10, 'active'),
    ('rel_011', 'entity_011', 'entity_012', 'competes_with', '竞争', '苹果与微软是长期竞争对手', 0.85, 0.90, 25, 22, 'active'),
    ('rel_012', 'entity_012', 'entity_013', 'partners_with', '合作', '微软与亚马逊有云计算合作', 0.70, 0.75, 18, 14, 'active'),

    -- 技术与应用的关系
    ('rel_020', 'entity_020', 'entity_021', 'includes', '包含', '人工智能包含机器学习', 0.95, 0.96, 40, 38, 'active'),
    ('rel_021', 'entity_021', 'entity_022', 'includes', '包含', '机器学习包含深度学习', 0.92, 0.94, 35, 33, 'active'),
    ('rel_022', 'entity_020', 'entity_040', 'enables', '使能', 'AI使能自动驾驶', 0.88, 0.90, 30, 27, 'active'),
    ('rel_023', 'entity_020', 'entity_043', 'includes', '包含', 'AI包含大语言模型', 0.90, 0.92, 32, 30, 'active'),
    ('rel_024', 'entity_021', 'entity_023', 'related_to', '相关', '机器学习与区块链相关', 0.55, 0.60, 15, 10, 'active'),

    -- 人物与产品的关系
    ('rel_030', 'entity_001', 'entity_031', 'uses', '使用', '埃隆·马斯克使用特斯拉Model S', 0.85, 0.88, 18, 16, 'active'),
    ('rel_031', 'entity_002', 'entity_030', 'created', '创造', '史蒂夫·乔布斯创造了iPhone', 0.95, 0.96, 25, 24, 'active'),
    ('rel_032', 'entity_003', 'entity_033', 'created', '创造', '比尔·盖茨创造了Windows', 0.95, 0.96, 28, 27, 'active'),

    -- 产品与技术的关系
    ('rel_040', 'entity_030', 'entity_020', 'powered_by', '驱动', 'iPhone由AI技术驱动', 0.75, 0.80, 20, 16, 'active'),
    ('rel_041', 'entity_031', 'entity_020', 'powered_by', '驱动', '特斯拉由AI技术驱动', 0.82, 0.85, 22, 18, 'active'),
    ('rel_042', 'entity_032', 'entity_043', 'powered_by', '驱动', 'ChatGPT由大语言模型驱动', 0.95, 0.96, 30, 29, 'active'),

    -- 概念之间的关系
    ('rel_050', 'entity_040', 'entity_020', 'depends_on', '依赖', '自动驾驶依赖人工智能', 0.90, 0.92, 25, 23, 'active'),
    ('rel_051', 'entity_041', 'entity_023', 'related_to', '相关', '云计算与区块链相关', 0.65, 0.70, 18, 13, 'active'),
    ('rel_052', 'entity_042', 'entity_020', 'related_to', '相关', '物联网与人工智能相关', 0.75, 0.80, 20, 16, 'active'),
    ('rel_053', 'entity_015', 'entity_043', 'provides', '提供', 'OpenAI提供大语言模型', 0.92, 0.94, 28, 26, 'active');

-- 更新实体的关系数量
UPDATE entities SET relation_count = (
    SELECT COUNT(*) FROM relations 
    WHERE relations.source_entity_id = entities.entity_id 
    OR relations.target_entity_id = entities.entity_id
);
