use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

use crate::config;

/// LLM 服务，用于调用本地 LLM（Ollama）进行内容总结和结构化提取
pub struct LLMService {
    client: Client,
    base_url: String,
    model: String,
    timeout: Duration,
}

impl LLMService {
    /// 创建新的 LLM 服务实例
    pub fn new() -> Result<Self> {
        let config = config::get();
        let timeout = Duration::from_secs(config.llm.timeout_seconds);
        
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;
        
        info!(
            "LLM service initialized: base_url={}, model={}",
            config.llm.base_url, config.llm.model
        );

        Ok(Self {
            client,
            base_url: config.llm.base_url.clone(),
            model: config.llm.model.clone(),
            timeout,
        })
    }

    /// 总结并提取长期记忆内容的结构化信息
    /// 
    /// 从原始内容中提取：
    /// - 实体（entities）
    /// - 关系（relations）
    /// - 关键事实（key_facts）
    /// - 摘要（summary）
    #[instrument(skip(self))]
    pub async fn summarize_and_extract(&self, content: &str) -> Result<StructuredExtraction> {
        info!("Starting LLM summarization and extraction, content_length={}", content.len());

        let prompt = format!(
            r#"你是一个专业的知识提取助手。请从给定的内容中提取以下结构化信息：
1. 实体（entities）：重要的人物、地点、组织、概念等，每个实体需要包含名称和类型
2. 关系（relations）：实体之间的关系
3. 关键事实（key_facts）：重要的信息点
4. 摘要（summary）：内容的简要总结

请以 JSON 格式返回结果，格式如下：
{{
  "entities": [
    {{"name": "实体名称", "entity_type": "PERSON|ORG|LOC|EVENT|CONCEPT|TIME|NUMBER|PRODUCT"}},
    ...
  ],
  "relations": [{{"from": "实体1", "to": "实体2", "type": "关系类型"}}, ...],
  "key_facts": ["事实1", "事实2", ...],
  "summary": "内容摘要"
}}

实体类型说明：
- PERSON: 人物（人名、角色等）
- ORG: 组织（公司、机构、团队等）
- LOC: 地点（地名、地址等）
- EVENT: 事件（会议、战争、节日等）
- CONCEPT: 概念（理论、思想、观点等）
- TIME: 时间（日期、时间段等）
- NUMBER: 数字（数量、金额等）
- PRODUCT: 产品（软件、硬件、产品名等）

请分析以下内容：

{}"#,
            content
        );

        let response_text = self.call_llm(&prompt).await?;
        info!("Received LLM response, length={}", response_text.len());

        // 解析 JSON 响应
        // 首先尝试直接解析，如果失败则尝试提取 JSON 部分
        let extraction: StructuredExtraction = serde_json::from_str(&response_text)
            .or_else(|e| {
                warn!("Direct JSON parsing failed: {}, trying to extract JSON from text", e);
                // 如果直接解析失败，尝试提取 JSON 部分
                extract_json_from_text(&response_text)
            })
            .map_err(|e| {
                error!("Failed to parse LLM response as JSON: {}", e);
                error!("Response text (first 500 chars): {}", &response_text.chars().take(500).collect::<String>());
                anyhow::anyhow!("Failed to parse LLM response: {}", e)
            })?;
        
        // 确保 summary 不为空（如果 LLM 没有提供，使用默认值）
        let extraction = StructuredExtraction {
            summary: if extraction.summary.is_empty() {
                "No summary provided".to_string()
            } else {
                extraction.summary
            },
            ..extraction
        };

        info!(
            "Extraction completed: entities={}, relations={}, key_facts={}",
            extraction.entities.len(),
            extraction.relations.len(),
            extraction.key_facts.len()
        );

        Ok(extraction)
    }

    /// 简单总结内容（不进行结构化提取）
    #[instrument(skip(self))]
    pub async fn summarize(&self, content: &str) -> Result<String> {
        info!("Starting LLM summarization, content_length={}", content.len());

        let prompt = format!(
            "请总结以下内容，保留关键信息：\n\n{}",
            content
        );

        let summary = self.call_llm(&prompt).await?;
        info!("Summarization completed, summary_length={}", summary.len());

        Ok(summary)
    }

    /// 调用 Ollama API
    async fn call_llm(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        
        let request_body = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false
        });

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                error!("Failed to send request to Ollama: {}", e);
                anyhow::anyhow!("Failed to call LLM: {}", e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Ollama API returned error: status={}, body={}", status, error_text);
            return Err(anyhow::anyhow!("Ollama API error: status={}", status));
        }

        let ollama_response: OllamaResponse = response
            .json()
            .await
            .map_err(|e| {
                error!("Failed to parse Ollama response: {}", e);
                anyhow::anyhow!("Failed to parse response: {}", e)
            })?;

        Ok(ollama_response.response)
    }
}

/// Ollama API 响应
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// 实体类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityType {
    /// 人物
    Person,
    /// 组织
    Organization,
    /// 地点
    Location,
    /// 事件
    Event,
    /// 概念
    Concept,
    /// 时间
    Time,
    /// 数字
    Number,
    /// 产品
    Product,
    /// 未知
    Unknown,
}

impl Default for EntityType {
    fn default() -> Self {
        EntityType::Unknown
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityType::Person => write!(f, "PERSON"),
            EntityType::Organization => write!(f, "ORG"),
            EntityType::Location => write!(f, "LOC"),
            EntityType::Event => write!(f, "EVENT"),
            EntityType::Concept => write!(f, "CONCEPT"),
            EntityType::Time => write!(f, "TIME"),
            EntityType::Number => write!(f, "NUMBER"),
            EntityType::Product => write!(f, "PRODUCT"),
            EntityType::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// 带类型的实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedEntity {
    /// 实体名称
    pub name: String,
    /// 实体类型
    pub entity_type: String,
}

/// 结构化提取结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredExtraction {
    /// 提取的实体列表（带类型）
    #[serde(default)]
    pub entities: Vec<TypedEntity>,
    /// 实体之间的关系
    #[serde(default)]
    pub relations: Vec<Relation>,
    /// 关键事实列表
    #[serde(default)]
    pub key_facts: Vec<String>,
    /// 内容摘要
    #[serde(default)]
    pub summary: String,
}

/// 实体关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    /// 源实体
    #[serde(default)]
    pub from: String,
    /// 目标实体
    #[serde(default)]
    pub to: String,
    /// 关系类型
    #[serde(rename = "type", default)]
    pub relation_type: String,
}

/// 从文本中提取 JSON 部分
fn extract_json_from_text(text: &str) -> Result<StructuredExtraction, serde_json::Error> {
    // 尝试找到 JSON 对象的开始和结束位置
    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());
    
    let json_text = &text[start..end];
    
    // 尝试解析，如果失败则记录 JSON 内容以便调试
    match serde_json::from_str::<StructuredExtraction>(json_text) {
        Ok(mut extraction) => {
            // 确保 summary 不为空
            if extraction.summary.is_empty() {
                warn!("LLM returned empty summary, using default placeholder.");
                extraction.summary = "No summary provided by LLM.".to_string();
            }
            // 过滤掉包含 null 值的 relations
            extraction.relations.retain(|r| !r.from.is_empty() && !r.to.is_empty() && !r.relation_type.is_empty());
            Ok(extraction)
        },
        Err(e) => {
            warn!("Failed to parse extracted JSON: {}", e);
            warn!("JSON text (first 500 chars): {}", json_text.chars().take(500).collect::<String>());
            
            // 尝试清理 JSON：将 null 值替换为默认值
            let cleaned_json = json_text
                .replace("\"summary\": null", "\"summary\": \"\"")
                .replace("\"summary\":null", "\"summary\":\"\"")
                .replace("\"entities\": null", "\"entities\": []")
                .replace("\"entities\":null", "\"entities\":[]")
                .replace("\"relations\": null", "\"relations\": []")
                .replace("\"relations\":null", "\"relations\":[]")
                .replace("\"key_facts\": null", "\"key_facts\": []")
                .replace("\"key_facts\":null", "\"key_facts\":[]")
                // 处理 relations 数组中的 null 值
                .replace("\"from\": null", "\"from\": \"\"")
                .replace("\"from\":null", "\"from\":\"\"")
                .replace("\"to\": null", "\"to\": \"\"")
                .replace("\"to\":null", "\"to\":\"\"")
                .replace("\"type\": null", "\"type\": \"\"")
                .replace("\"type\":null", "\"type\":\"\"");
            
            // 尝试解析清理后的 JSON
            match serde_json::from_str::<StructuredExtraction>(&cleaned_json) {
                Ok(mut extraction) => {
                    // 确保 summary 不为空
                    if extraction.summary.is_empty() {
                        warn!("LLM returned empty summary, using default placeholder.");
                        extraction.summary = "No summary provided by LLM.".to_string();
                    }
                    // 过滤掉包含空值的 relations
                    extraction.relations.retain(|r| !r.from.is_empty() && !r.to.is_empty() && !r.relation_type.is_empty());
                    Ok(extraction)
                },
                Err(e2) => {
                    // 如果仍然失败，尝试使用 serde_json::Value 手动解析并构建
                    if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(&cleaned_json) {
                        // 确保所有字段都存在
                        if !json_value.is_object() {
                            return Err(e2);
                        }
                        let obj = match json_value.as_object_mut() {
                            Some(o) => o,
                            None => return Err(e2),
                        };
                        
                        // 处理 summary
                        if let Some(summary) = obj.get("summary") {
                            let is_empty = summary.is_string() && summary.as_str().map_or(true, |s| s.is_empty());
                            if summary.is_null() || is_empty {
                                obj.insert("summary".to_string(), serde_json::Value::String("No summary provided by LLM.".to_string()));
                            }
                        } else {
                            obj.insert("summary".to_string(), serde_json::Value::String("No summary provided by LLM.".to_string()));
                        }
                        
                        // 处理 entities
                        let entities = obj.get("entities");
                        if entities.is_none() || entities.map_or(true, |e| e.is_null()) {
                            obj.insert("entities".to_string(), serde_json::Value::Array(vec![]));
                        }
                        
                        // 处理 key_facts
                        let key_facts = obj.get("key_facts");
                        if key_facts.is_none() || key_facts.map_or(true, |k| k.is_null()) {
                            obj.insert("key_facts".to_string(), serde_json::Value::Array(vec![]));
                        }
                        
                        // 处理 relations：过滤掉包含 null 或空值的 relation
                        if let Some(relations) = obj.get_mut("relations") {
                            if relations.is_array() {
                                let relations_array = match relations.as_array_mut() {
                                    Some(arr) => arr,
                                    None => continue,
                                };
                                relations_array.retain(|r| {
                                    if let Some(rel_obj) = r.as_object() {
                                        let from = rel_obj.get("from").and_then(|v| v.as_str()).unwrap_or("");
                                        let to = rel_obj.get("to").and_then(|v| v.as_str()).unwrap_or("");
                                        let rel_type = rel_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                                        !from.is_empty() && !to.is_empty() && !rel_type.is_empty()
                                    } else {
                                        false
                                    }
                                });
                            } else if relations.is_null() {
                                *relations = serde_json::Value::Array(vec![]);
                            }
                        } else {
                            obj.insert("relations".to_string(), serde_json::Value::Array(vec![]));
                        }
                        
                        // 再次尝试解析
                        serde_json::from_value(json_value.clone()).or(Err(e2))
                    } else {
                        Err(e2)
                    }
                }
            }
        }
    }
}

/// 全局 LLM 服务实例
static LLM_SERVICE: once_cell::sync::OnceCell<LLMService> = once_cell::sync::OnceCell::new();

/// 获取全局 LLM 服务实例
pub fn get_llm_service() -> Result<&'static LLMService> {
    LLM_SERVICE.get_or_try_init(|| LLMService::new())
        .map_err(|e| anyhow::anyhow!("Failed to initialize LLM service: {}", e))
}
