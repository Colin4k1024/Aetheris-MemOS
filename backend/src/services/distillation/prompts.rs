pub const L1_EXTRACTION_SYSTEM_PROMPT: &str = r#"你是专业的"情境切分与记忆提取专家"。
你的任务是分析用户的对话，判断情境切换，并从中提取结构化的核心记忆（仅限 persona, episodic, instruction 三类）。

**输出语言**：所有自由文本字段（scene_name、memory content）使用与用户消息相同的语言；JSON 字段名、枚举值保持英文。

### 任务一：情境切分（Scene Segmentation）
分析【待提取的新消息】，结合【上一个情境】，判断并输出当前对话的情境。
- 继承：无明显切换，沿用上一个情境。
- 切换条件：用户发出明确指令（如"换话题"）、意图转变、或提出独立新目标。
- 命名规则：简短描述当前活动场景（约30-50字符）。

### 任务二：核心记忆提取（Memory Extraction）
仅从【待提取的新消息】中提取核心信息。

【通用提取原则】
1. 宁缺毋滥：过滤琐碎闲聊、临时性指令和一次性操作。
2. 独立完整：记忆必须"跳出当前对话依然成立"，无上下文也能看懂。
3. 归纳合并：强关联的多条消息，必须合并为一条完整记忆。

【支持提取的三大类型】
1. persona：用户的稳定属性、偏好、技能、价值观、习惯。
   - 打分：80-100（核心特质）；50-70（一般喜好）；<50（丢弃）。

2. episodic：客观发生的动作、决定、计划或达成结果。
   - 打分：80-100（重要事件）；60-70（一般活动）；<60（丢弃）。

3. instruction：用户对 AI 提出的长期行为规则、格式偏好。
   - 打分：-1（最高优先级死命令）；90-100（核心规则）；70-80（重要要求）；<70（丢弃）。

### 不应该提取的内容
- 琐碎闲聊、问候；临时性的纯工具性请求
- 重复的内容；AI助手自身的行为或输出
- 纯主观感受（不带客观事件的情绪表达）

### 输出格式（JSON）
返回且仅返回一个合法的 JSON 数组：

[
  {
    "scene_name": "当前情境名称",
    "message_ids": ["属于该情境的消息ID"],
    "memories": [
      {
        "content": "完整、独立的记忆陈述",
        "type": "persona|episodic|instruction",
        "priority": 80,
        "source_message_ids": ["消息ID"],
        "metadata": {}
      }
    ]
  }
]

如果无有意义的记忆，memories 为空数组。"#;

pub fn format_l1_extraction_user_prompt(
    previous_scene: &str,
    background_messages: &str,
    new_messages: &str,
) -> String {
    format!(
        r#"【上一个情境】
{previous_scene}

【背景消息（仅供参考，不要从中提取记忆）】
{background_messages}

【待提取的新消息】
{new_messages}"#,
    )
}

pub const L1_DEDUP_SYSTEM_PROMPT: &str = r#"你是记忆冲突检测专家。判断新记忆与已有记忆是否冲突或重复。

对于每条新记忆，与其候选已有记忆对比，做出以下判断之一：
- keep：新记忆是全新信息，直接保留
- merge：新记忆与某条已有记忆高度相关，应合并为一条更完整的记忆
- supersede：新记忆是已有记忆的更新版本，应替换旧记忆
- discard：新记忆与已有记忆完全重复，丢弃

返回 JSON 数组，每条新记忆对应一个判断：
[
  {
    "decision": "keep|merge|supersede|discard",
    "existing_id": "如果是 merge/supersede，填已有记忆的 ID",
    "merged_content": "如果是 merge，填合并后的新内容"
  }
]"#;

pub fn format_l1_dedup_user_prompt(
    new_memories: &str,
    existing_candidates: &str,
) -> String {
    format!(
        r#"【新提取的记忆】
{new_memories}

【候选已有记忆（按相似度排序）】
{existing_candidates}"#,
    )
}

pub const L2_CONSOLIDATION_SYSTEM_PROMPT: &str = r#"你是记忆整合架构师。你的目标是将碎片化的 L1 记忆原子整合成连贯的场景叙事文档。

输入：一批新的记忆原子（带 scene_name 标签）和现有的场景文档摘要列表。

你的任务：
1. 将新记忆归类到合适的场景文档中
2. 如果没有合适的场景，创建新场景
3. 每个场景文档应该是一篇连贯的叙事，不是简单的列表

场景文档格式：
- 用 Markdown 格式
- 包含章节分类（如"用户核心特征"、"工作习惯"、"技术偏好"等）
- 每个场景不超过 2000 字
- 场景总数不超过 15 个

返回 JSON：
[
  {
    "scene_id": "已有场景ID 或 'new'",
    "name": "场景名称",
    "summary": "一句话摘要",
    "content": "完整的 Markdown 场景叙事",
    "atom_ids": ["归入此场景的原子ID"]
  }
]"#;

pub fn format_l2_consolidation_user_prompt(
    new_atoms: &str,
    existing_scenes: &str,
) -> String {
    format!(
        r#"【新的记忆原子】
{new_atoms}

【现有场景文档摘要】
{existing_scenes}"#,
    )
}

pub const L3_PERSONA_SYSTEM_PROMPT: &str = r#"你是用户画像生成专家。根据所有场景文档，生成或更新用户的综合画像。

画像应包含：
1. 基本信息：角色、职业、技术栈
2. 核心偏好：工作风格、沟通方式、技术倾向
3. 行为模式：常见工作流程、决策习惯
4. 长期指令：用户对AI的持久性要求
5. 关键上下文：当前项目、团队环境、约束条件

要求：
- 用 Markdown 格式
- 简洁但全面（不超过 1500 字）
- 只包含有证据支撑的信息
- 标注信息的可信度（高/中/低）

直接返回 Markdown 格式的画像文档，不要包装在 JSON 中。"#;

pub fn format_l3_persona_user_prompt(
    existing_persona: &str,
    scene_contents: &str,
) -> String {
    format!(
        r#"【现有画像（如有）】
{existing_persona}

【所有场景文档】
{scene_contents}"#,
    )
}
