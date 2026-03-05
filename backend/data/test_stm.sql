-- =============================================
-- 短期记忆测试数据 (Short-Term Memory Test Data)
-- =============================================

-- 会话1: 简单问答对话
INSERT INTO context_sessions (session_id, user_id, agent_id, session_type, max_context_length, expires_at, status, priority)
VALUES
    ('session_001', 'user_001', 'agent_001', 'conversation', 4096, datetime('now', '+24 hours'), 'active', 8);

INSERT INTO context_messages (message_id, session_id, message_index, role, content, content_type, content_length, token_count, is_processed, is_important, retention_score)
VALUES
    ('msg_001_001', 'session_001', 0, 'system', '你是一个有帮助的AI助手。', 'text', 20, 8, 1, 1, 0.95),
    ('msg_001_002', 'session_001', 1, 'user', '请介绍一下北京的历史。', 'text', 18, 12, 1, 0, 0.65),
    ('msg_001_003', 'session_001', 2, 'assistant', '北京是中国的首都，有着三千多年的建城史和八百多年的建都史。最早可以追溯到西周时期的燕国...', 'text', 85, 65, 1, 1, 0.85),
    ('msg_001_004', 'session_001', 3, 'user', '那故宫是什么时候建的？', 'text', 18, 15, 1, 0, 0.60),
    ('msg_001_005', 'session_001', 4, 'assistant', '故宫始建于明成祖永乐四年(1406年)，于永乐十八年(1420年)建成。', 'text', 58, 42, 1, 1, 0.80);

-- 会话2: 复杂推理任务
INSERT INTO context_sessions (session_id, user_id, agent_id, session_type, max_context_length, expires_at, status, priority)
VALUES
    ('session_002', 'user_002', 'agent_002', 'task', 8192, datetime('now', '+48 hours'), 'active', 9);

INSERT INTO context_messages (message_id, session_id, message_index, role, content, content_type, content_length, token_count, is_processed, is_important, retention_score)
VALUES
    ('msg_002_001', 'session_002', 0, 'system', '你是一个专业的编程助手，擅长解决复杂的算法问题。', 'text', 35, 18, 1, 1, 0.95),
    ('msg_002_002', 'session_002', 1, 'user', '请用Python实现一个二分查找算法，要求处理边界情况。', 'text', 48, 35, 1, 1, 0.90),
    ('msg_002_003', 'session_002', 2, 'assistant', '```python\ndef binary_search(arr, target):\n    if not arr:\n        return -1\n    \n    left, right = 0, len(arr) - 1\n    \n    while left <= right:\n        mid = (left + right) // 2\n        \n        if arr[mid] == target:\n            return mid\n        elif arr[mid] < target:\n            left = mid + 1\n        else:\n            right = mid - 1\n    \n    return -1\n```', 'markdown', 380, 180, 1, 1, 0.92),
    ('msg_002_004', 'session_002', 3, 'user', '如何处理浮点数的比较？', 'text', 22, 18, 1, 0, 0.55),
    ('msg_002_005', 'session_002', 4, 'assistant', '浮点数比较应该使用误差范围：\n\n```python\nimport math\n\ndef is_equal(a, b, epsilon=1e-9):\n    return math.fabs(a - b) < epsilon\n```', 'markdown', 165, 95, 1, 1, 0.85);

-- 会话3: 多模态任务
INSERT INTO context_sessions (session_id, user_id, agent_id, session_type, max_context_length, expires_at, status, priority)
VALUES
    ('session_003', 'user_003', 'agent_003', 'query', 4096, datetime('now', '+12 hours'), 'active', 7);

INSERT INTO context_messages (message_id, session_id, message_index, role, content, content_type, content_length, token_count, is_processed, is_important, retention_score)
VALUES
    ('msg_003_001', 'session_003', 0, 'user', '[图片] 这张图片描述的是什么场景？', 'text', 32, 25, 1, 1, 0.75),
    ('msg_003_002', 'session_003', 1, 'assistant', '图片显示的是一个现代化的城市天际线，可能是上海陆家嘴金融贸易区。', 'text', 58, 42, 1, 1, 0.80),
    ('msg_003_003', 'session_003', 2, 'user', '请介绍一下陆家嘴的发展历史。', 'text', 22, 18, 1, 0, 0.60),
    ('msg_003_004', 'session_003', 3, 'assistant', '陆家嘴位于上海浦东新区，是中国的金融中心之一。1990年代开始大规模开发...', 'text', 75, 55, 1, 1, 0.82);

-- 会话4: 知识问答
INSERT INTO context_sessions (session_id, user_id, agent_id, session_type, max_context_length, expires_at, status, priority)
VALUES
    ('session_004', 'user_004', 'agent_001', 'conversation', 4096, datetime('now', '+24 hours'), 'completed', 6);

INSERT INTO context_messages (message_id, session_id, message_index, role, content, content_type, content_length, token_count, is_processed, is_important, retention_score)
VALUES
    ('msg_004_001', 'session_004', 0, 'user', '什么是机器学习？', 'text', 18, 12, 1, 0, 0.50),
    ('msg_004_002', 'session_004', 1, 'assistant', '机器学习是人工智能的一个分支，它使计算机能够从数据中学习并改进性能，而无需明确编程。', 'text', 65, 48, 1, 1, 0.78),
    ('msg_004_003', 'session_004', 2, 'user', '有哪些常见的机器学习算法？', 'text', 22, 16, 1, 0, 0.52),
    ('msg_004_004', 'session_004', 3, 'assistant', '常见的机器学习算法包括：\n1. 监督学习：线性回归、逻辑回归、决策树、随机森林、SVM、神经网络\n2. 无监督学习：K-means聚类、层次聚类、主成分分析(PCA)\n3. 强化学习：Q-learning、Deep Q Network', 'text', 180, 125, 1, 1, 0.88);

-- 会话5: 代码调试
INSERT INTO context_sessions (session_id, user_id, agent_id, session_type, max_context_length, expires_at, status, priority)
VALUES
    ('session_005', 'user_005', 'agent_002', 'task', 8192, datetime('now', '+72 hours'), 'active', 10);

INSERT INTO context_messages (message_id, session_id, message_index, role, content, content_type, content_length, token_count, is_processed, is_important, retention_score)
VALUES
    ('msg_005_001', 'session_005', 0, 'user', '我的Python代码运行报错：IndexError: list index out of range', 'text', 52, 38, 1, 1, 0.85),
    ('msg_005_002', 'session_005', 1, 'assistant', '请分享你的代码，这样我可以帮你定位问题。', 'text', 28, 20, 1, 0, 0.70),
    ('msg_005_003', 'session_005', 2, 'user', '```python\nfor i in range(len(arr)):\n    print(arr[i+1])\n```', 'code', 55, 28, 1, 1, 0.90),
    ('msg_005_004', 'session_005', 3, 'assistant', '问题在于 arr[i+1]，当 i 是最后一个索引时，i+1 会超出范围。修复方法：\n\n```python\nfor i in range(len(arr) - 1):\n    print(arr[i+1])\n\n# 或者使用enumerate\nfor i, item in enumerate(arr[1:], 1):\n    print(item)\n```', 'markdown', 220, 145, 1, 1, 0.95);

-- 更新会话的上下文长度
UPDATE context_sessions SET context_length = (SELECT COUNT(*) FROM context_messages WHERE session_id = context_sessions.session_id) * 100;
