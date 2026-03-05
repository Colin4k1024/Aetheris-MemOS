-- =============================================
-- 长期记忆测试数据 (Long-Term Memory Test Data)
-- =============================================

-- 1. 知识条目 - 科技类
INSERT INTO knowledge_entries (entry_id, source_id, source_type, title, content, content_type, content_hash, embedding_vector, embedding_model, embedding_dimension, category, domain, quality_score, relevance_score, confidence_score, access_count, success_count, status)
VALUES
    ('ltm_001', 'doc_001', 'document', 'Python 3.12 新特性', 'Python 3.12 引入了多项新特性，包括：\n1. 性能优化：解释器启动速度提升10%\n2. f-string 支持更复杂的调试表达式\n3. 改进的错误信息提示\n4. 支持 PEP 654 的异常组(Exception Groups)\n5. 新的类型标注语法', 'markdown', 'abc123def456', '[0.1, 0.2, 0.3, 0.4, 0.5]', 'nomic-embed-text', 768, '编程语言', '技术', 0.92, 0.88, 0.95, 150, 142, 'active'),

    ('ltm_002', 'doc_002', 'document', 'React 19 指南', 'React 19 是 React 的重大更新，主要特性包括：\n1. Server Components：服务端组件\n2. Actions：表单处理简化\n3. use() Hook：简化异步数据获取\n4. 改进的 Suspense\n5. 编译器自动优化', 'markdown', 'def456ghi789', '[0.2, 0.3, 0.4, 0.5, 0.6]', 'nomic-embed-text', 768, '前端框架', '技术', 0.90, 0.85, 0.92, 200, 185, 'active'),

    ('ltm_003', 'doc_003', 'document', 'Rust 编程范式', 'Rust 是一种系统编程语言，强调：\n1. 内存安全：所有权系统\n2. 零成本抽象\n3. 并发安全：借用检查器\n4. 模式匹配\n5. 强大的类型系统', 'text', 'ghi789jkl012', '[0.3, 0.4, 0.5, 0.6, 0.7]', 'nomic-embed-text', 768, '编程语言', '技术', 0.88, 0.82, 0.90, 120, 108, 'active'),

    ('ltm_004', 'web_001', 'web', '机器学习基础', '机器学习是人工智能的一个分支，专注于让计算机从数据中学习。\n主要类型：\n- 监督学习：从标注数据学习\n- 无监督学习：从无标注数据发现模式\n- 强化学习：通过与环境交互学习', 'text', 'jkl012mno345', '[0.4, 0.5, 0.6, 0.7, 0.8]', 'nomic-embed-text', 768, '人工智能', '技术', 0.95, 0.92, 0.98, 300, 290, 'active'),

    ('ltm_005', 'doc_004', 'document', 'Docker 最佳实践', '使用 Docker 的最佳实践：\n1. 使用多阶段构建减小镜像体积\n2. 合理分层利用缓存\n3. 使用 .dockerignore 排除不必要的文件\n4. 以非 root 用户运行容器\n5. 及时清理无用数据', 'markdown', 'mno345pqr678', '[0.5, 0.6, 0.7, 0.8, 0.9]', 'nomic-embed-text', 768, '容器化', '技术', 0.85, 0.80, 0.88, 80, 72, 'active');

-- 2. 知识条目 - 金融类
INSERT INTO knowledge_entries (entry_id, source_id, source_type, title, content, content_type, content_hash, embedding_vector, embedding_model, embedding_dimension, category, domain, quality_score, relevance_score, confidence_score, access_count, success_count, status)
VALUES
    ('ltm_006', 'doc_005', 'document', '股票投资基础知识', '股票投资的基本概念：\n1. 市盈率(P/E)：衡量股票价格相对于收益的指标\n2. 市净率(P/B)：衡量股票价格相对于账面价值的指标\n3. 股息率：股票每年派息与价格的比率\n4. 每股收益(EPS)：公司净利润除以股本数', 'text', 'pqr678stu901', '[0.1, 0.15, 0.2, 0.25, 0.3]', 'nomic-embed-text', 768, '投资理财', '金融', 0.82, 0.78, 0.85, 95, 82, 'active'),

    ('ltm_007', 'doc_006', 'document', '区块链技术原理', '区块链是一种分布式账本技术：\n1. 去中心化：无单一机构控制\n2. 不可篡改：密码学保证\n3. 可追溯：全链路可查\n4. 智能合约：自动执行代码\n5. 共识机制：PoW、PoS、DPoS', 'text', 'stu901vwx234', '[0.15, 0.2, 0.25, 0.3, 0.35]', 'nomic-embed-text', 768, '区块链', '金融', 0.88, 0.84, 0.90, 150, 138, 'active'),

    ('ltm_008', 'api_001', 'api', 'RESTful API 设计原则', 'RESTful API 设计最佳实践：\n1. 使用名词而非动词表示资源\n2. 使用 HTTP 方法语义(GET/POST/PUT/DELETE)\n3. 返回适当的 HTTP 状态码\n4. 版本化 API\n5. 使用分页处理大量数据', 'markdown', 'vwx234yza567', '[0.2, 0.25, 0.3, 0.35, 0.4]', 'nomic-embed-text', 768, 'API设计', '技术', 0.92, 0.88, 0.94, 220, 208, 'active');

-- 3. 知识条目 - 日常生活类
INSERT INTO knowledge_entries (entry_id, source_id, source_type, title, content, content_type, content_hash, embedding_vector, embedding_model, embedding_dimension, category, domain, quality_score, relevance_score, confidence_score, access_count, success_count, status)
VALUES
    ('ltm_009', 'user_001', 'user_input', '咖啡冲泡指南', '冲泡美味咖啡的步骤：\n1. 选用新鲜咖啡豆，研磨粗细适中\n2. 水温控制在90-96度\n3. 水粉比例约1:15-1:17\n4. 萃取时间2-4分钟\n5. 建议使用过滤水', 'text', 'yza567bcd890', '[0.6, 0.65, 0.7, 0.75, 0.8]', 'nomic-embed-text', 768, '生活技巧', '生活', 0.75, 0.70, 0.78, 45, 38, 'active'),

    ('ltm_010', 'doc_007', 'document', '时间管理方法', '有效的时间管理技巧：\n1. 番茄工作法：25分钟专注+5分钟休息\n2. 艾森豪威尔矩阵：按重要/紧急分类任务\n3. GTD：Getting Things Done 方法\n4. 早起鸟 vs 夜猫子：找到最佳工作时间\n5. 定期复盘优化', 'text', 'bcd890efg123', '[0.65, 0.7, 0.75, 0.8, 0.85]', 'nomic-embed-text', 768, '自我提升', '生活', 0.88, 0.85, 0.90, 180, 165, 'active'),

    ('ltm_011', 'web_002', 'web', '健康饮食建议', '均衡饮食的原则：\n1. 多摄入蔬果（每天5份以上）\n2. 适量优质蛋白（鱼、禽、豆类）\n3. 控制糖和盐的摄入\n4. 保持充足水分（每天2升）\n5. 定时定量，少吃夜宵', 'text', 'efg123hij456', '[0.7, 0.75, 0.8, 0.85, 0.9]', 'nomic-embed-text', 768, '健康', '生活', 0.90, 0.88, 0.92, 250, 235, 'active');

-- 4. 知识条目 - 行业知识
INSERT INTO knowledge_entries (entry_id, source_id, source_type, title, content, content_type, content_hash, embedding_vector, embedding_model, embedding_dimension, category, domain, quality_score, relevance_score, confidence_score, access_count, success_count, status)
VALUES
    ('ltm_012', 'doc_008', 'document', 'SaaS 产品设计原则', 'SaaS 产品设计关键点：\n1. 用户体验优先：简洁易用\n2. 订阅模式：稳定的收入来源\n3. 数据安全：加密、备份、权限管理\n4. 可扩展性：支持业务增长\n5. 多租户架构：资源隔离与共享', 'markdown', 'hij456klm789', '[0.3, 0.35, 0.4, 0.45, 0.5]', 'nomic-embed-text', 768, '产品设计', '商业', 0.86, 0.82, 0.88, 110, 98, 'active'),

    ('ltm_013', 'doc_009', 'document', '云计算服务模型', '云计算三大服务模型：\n1. IaaS（基础设施即服务）：虚拟机、存储、网络\n2. PaaS（平台即服务）：开发平台、数据库\n3. SaaS（软件即服务）：在线应用\n\n主要提供商：AWS、Azure、GCP、阿里云', 'text', 'klm789nop012', '[0.35, 0.4, 0.45, 0.5, 0.55]', 'nomic-embed-text', 768, '云计算', '技术', 0.92, 0.88, 0.94, 175, 162, 'active'),

    ('ltm_014', 'db_001', 'database', 'PostgreSQL vs MySQL 对比', 'PostgreSQL vs MySQL:\n\nPostgreSQL:\n- 更强大的功能（CTE、窗口函数）\n- 更好的标准遵循\n- 更强的扩展性\n- 适合复杂查询\n\nMySQL:\n- 简单易用\n- 更高的写入性能\n- 更成熟的生态\n- 适合Web应用', 'text', 'nop012qrs345', '[0.4, 0.45, 0.5, 0.55, 0.6]', 'nomic-embed-text', 768, '数据库', '技术', 0.88, 0.84, 0.90, 130, 118, 'active');

-- 5. 知识条目 - 历史与文化
INSERT INTO knowledge_entries (entry_id, source_id, source_type, title, content, content_type, content_hash, embedding_vector, embedding_model, embedding_dimension, category, domain, quality_score, relevance_score, confidence_score, access_count, success_count, status)
VALUES
    ('ltm_015', 'web_003', 'web', '丝绸之路历史', '丝绸之路是古代连接东西方的贸易通道：\n1. 始于汉代（公元前206年）\n2. 路线：从长安到地中海\n3. 贸易商品：丝绸、茶叶、瓷器、香料\n4. 文化传播：佛教、伊斯兰教东传\n5. 历史意义：东西方文明交流', 'text', 'qrs345tuv678', '[0.8, 0.82, 0.84, 0.86, 0.88]', 'nomic-embed-text', 768, '历史', '文化', 0.92, 0.90, 0.95, 85, 80, 'active'),

    ('ltm_016', 'doc_010', 'document', '日本茶道文化', '日本茶道（茶の湯）:\n1. 起源：中国唐代传入\n2. 发展：室町时代形成现代形式\n3. 核心：和敬清寂\n4. 茶具：茶碗、茶勺、茶入等\n5. 场所：茶室、露地', 'text', 'tuv678wxy901', '[0.82, 0.84, 0.86, 0.88, 0.9]', 'nomic-embed-text', 768, '文化', '人文', 0.88, 0.85, 0.90, 60, 55, 'active');

-- 知识条目之间的关联
INSERT INTO knowledge_relations (relation_id, source_entry_id, target_entry_id, relation_type, relation_strength, relation_confidence, metadata)
VALUES
    ('rel_001', 'ltm_001', 'ltm_003', 'related', 0.75, 0.80, '{"topic": "编程语言"}'),
    ('rel_002', 'ltm_002', 'ltm_004', 'related', 0.70, 0.75, '{"topic": "前端与AI"}'),
    ('rel_003', 'ltm_003', 'ltm_005', 'supports', 0.65, 0.72, '{"topic": "系统编程与容器"}'),
    ('rel_004', 'ltm_006', 'ltm_007', 'related', 0.68, 0.70, '{"topic": "金融科技"}'),
    ('rel_005', 'ltm_008', 'ltm_013', 'related', 0.80, 0.85, '{"topic": "API与云计算"}'),
    ('rel_006', 'ltm_010', 'ltm_011', 'supports', 0.72, 0.78, '{"topic": "生活与健康"}'),
    ('rel_007', 'ltm_015', 'ltm_016', 'related', 0.65, 0.70, '{"topic": "文化交流"}');
