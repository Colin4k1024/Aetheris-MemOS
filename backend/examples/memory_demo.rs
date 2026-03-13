#![allow(dead_code)]

/// 记忆系统完整演示程序
///
/// 本程序演示自适应记忆系统的所有核心功能：
/// 1. 短期记忆(STM) - 会话管理、消息存储
/// 2. 长期记忆(LTM) - 知识条目存储、向量搜索
/// 3. 记忆调度 - 任务分析、性能预测
///
/// 运行方式：
///   1. 启动服务: cd backend && cargo run
///   2. 运行演示: cargo run --example memory_demo
///
/// API服务地址: http://localhost:8008
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
struct DemoContext {
    client: Client,
    base_url: String,
}

impl DemoContext {
    fn new(base_url: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.to_string(),
        }
    }
}

// ============================================================================
// 数据结构定义 - 与实际API匹配
// ============================================================================

// STM - 短期记忆
#[derive(Serialize)]
struct StoreStmRequest {
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "agentId")]
    agent_id: String,
    #[serde(rename = "sessionType")]
    session_type: String,
    role: String,
    content: String,
    #[serde(rename = "maxContextLength")]
    max_context_length: Option<i32>,
    #[serde(rename = "retentionHours")]
    retention_hours: Option<i32>,
}

#[derive(Deserialize)]
struct StoreStmResponse {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "messageId")]
    message_id: String,
}

// LTM - 长期记忆
#[derive(Serialize)]
struct StoreLtmRequest {
    #[serde(rename = "sourceId")]
    source_id: String,
    #[serde(rename = "sourceType")]
    source_type: String,
    content: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct StoreLtmResponse {
    #[serde(rename = "entryId")]
    entry_id: String,
}

// 搜索
#[derive(Serialize)]
struct SearchRequest {
    query: String,
    #[serde(rename = "topK")]
    top_k: Option<usize>,
}

#[derive(Deserialize)]
struct SearchResult {
    #[serde(rename = "entryId")]
    entry_id: Option<String>,
    score: f32,
    content: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

// 资源
#[derive(Deserialize)]
struct ResourcesResponse {
    cpu: f64,
    memory: f64,
}

// 分析 - 正确的请求结构
#[derive(Serialize)]
struct TaskContextInput {
    content: String,
    modality: Vec<String>,
}

#[derive(Serialize)]
struct AnalyzeRequest {
    #[serde(rename = "task_context")]
    task_context: TaskContextInput,
}

#[derive(Deserialize)]
struct AnalyzeResponse {
    characteristics: serde_json::Value,
    #[serde(rename = "memoryStrategy")]
    memory_strategy: serde_json::Value,
    #[serde(rename = "confidenceScore")]
    confidence_score: f64,
}

// 预测 - 正确的请求结构
#[derive(Serialize)]
struct TaskProfile {
    #[serde(rename = "taskType")]
    task_type: String,
    complexity: String,
    #[serde(rename = "reasoningDepth")]
    reasoning_depth: String,
    domain: Option<String>,
}

#[derive(Serialize)]
struct PredictRequest {
    #[serde(rename = "memoryConfig")]
    memory_config: serde_json::Value,
    #[serde(rename = "taskProfile")]
    task_profile: TaskProfile,
}

#[derive(Deserialize)]
struct PredictResponse {
    #[serde(rename = "efficiencyGain")]
    efficiency_gain: f64,
    #[serde(rename = "coherenceGain")]
    coherence_gain: f64,
    #[serde(rename = "resourceCost")]
    resource_cost: f64,
}

// ============================================================================
// 演示函数
// ============================================================================

async fn demo_stm(ctx: &DemoContext) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("演示1: 短期记忆 (Short-Term Memory)");
    println!("{}\n", "=".repeat(60));

    // 1.1 创建会话并添加消息
    println!("1.1 创建会话并添加消息...");
    let req = StoreStmRequest {
        user_id: "demo_user".to_string(),
        agent_id: "demo_agent".to_string(),
        session_type: "conversation".to_string(),
        role: "user".to_string(),
        content: "请给我讲讲人工智能的发展历史".to_string(),
        max_context_length: Some(4096),
        retention_hours: Some(24),
    };

    let response = ctx
        .client
        .post(format!("{}/api/v1/memory/storage/stm", ctx.base_url))
        .json(&req)
        .send()
        .await?;

    if response.status().is_success() {
        let result: StoreStmResponse = response.json().await?;
        println!("✓ 会话创建成功: session_id = {}", result.session_id);
        println!("✓ 消息添加成功: message_id = {}", result.message_id);

        // 1.2 添加助手回复
        println!("\n1.2 添加助手回复...");
        let req2 = StoreStmRequest {
            user_id: "demo_user".to_string(),
            agent_id: "demo_agent".to_string(),
            session_type: "conversation".to_string(),
            role: "assistant".to_string(),
            content: "人工智能的发展可以追溯到1956年的达特茅斯会议，经历了符号主义、连接主义、深度学习等多个阶段。".to_string(),
            max_context_length: Some(4096),
            retention_hours: Some(24),
        };

        let response = ctx
            .client
            .post(format!("{}/api/v1/memory/storage/stm", ctx.base_url))
            .json(&req2)
            .send()
            .await?;

        if response.status().is_success() {
            let result: StoreStmResponse = response.json().await?;
            println!("✓ 助手回复添加成功: message_id = {}", result.message_id);
        }
    } else {
        println!(
            "✗ 创建会话失败: {:?}",
            response.text().await.unwrap_or_default()
        );
    }

    Ok(())
}

async fn demo_ltm(ctx: &DemoContext) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("演示2: 长期记忆 (Long-Term Memory)");
    println!("{}\n", "=".repeat(60));

    // 2.1 存储知识条目
    println!("2.1 存储知识条目...");

    let knowledge_entries = vec![
        (
            "doc_python",
            "Python编程",
            "Python是一种高级编程语言,具有简洁易读的语法和丰富的库支持。",
        ),
        (
            "doc_rust",
            "Rust系统编程",
            "Rust是一种系统编程语言,强调内存安全和并发性能。",
        ),
        (
            "doc_ml",
            "机器学习基础",
            "机器学习是人工智能的分支,让计算机从数据中学习并改进。",
        ),
    ];

    for (source_id, title, content) in knowledge_entries {
        let req = StoreLtmRequest {
            source_id: source_id.to_string(),
            source_type: "document".to_string(),
            title: Some(title.to_string()),
            content: content.to_string(),
        };

        let response = ctx
            .client
            .post(format!("{}/api/v1/memory/storage/ltm", ctx.base_url))
            .json(&req)
            .send()
            .await?;

        if response.status().is_success() {
            let result: StoreLtmResponse = response.json().await?;
            println!(
                "✓ 知识条目存储成功: entry_id = {}, title = {}",
                result.entry_id, title
            );
        } else {
            println!(
                "✗ 存储知识失败: {:?}",
                response.text().await.unwrap_or_default()
            );
        }
    }

    // 2.2 搜索知识 (需要Qdrant服务运行)
    println!("\n2.2 搜索知识条目 (需要Qdrant服务)...");
    let search_req = SearchRequest {
        query: "神经网络 深度学习".to_string(),
        top_k: Some(3),
    };

    let response = ctx
        .client
        .post(format!("{}/api/v1/memory/search/ltm", ctx.base_url))
        .json(&search_req)
        .send()
        .await?;

    if response.status().is_success() {
        let result: SearchResponse = response.json().await?;
        println!("✓ 搜索到 {} 条相关知识:", result.results.len());
        for (i, r) in result.results.iter().enumerate() {
            println!(
                "  {}. [score: {:.3}] {}",
                i + 1,
                r.score,
                r.title.as_deref().unwrap_or("N/A")
            );
        }
    } else {
        println!(
            "✗ 搜索失败 (需要启动Qdrant服务): {:?}",
            response.text().await.unwrap_or_default()
        );
    }

    Ok(())
}

async fn demo_analysis(ctx: &DemoContext) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("演示3: 自适应记忆调度");
    println!("{}\n", "=".repeat(60));

    // 3.1 分析任务特征
    println!("3.1 分析任务特征...");
    let req = AnalyzeRequest {
        task_context: TaskContextInput {
            content: "用户询问如何学习编程,需要给出详细的学习路线和建议".to_string(),
            modality: vec!["text".to_string()],
        },
    };

    let response = ctx
        .client
        .post(format!(
            "{}/api/v1/memory/analyzer/task-characteristics",
            ctx.base_url
        ))
        .json(&req)
        .send()
        .await?;

    if response.status().is_success() {
        let result: AnalyzeResponse = response.json().await?;
        println!("✓ 任务分析成功:");
        println!("  - 置信度: {:.2}", result.confidence_score);
    } else {
        println!(
            "✗ 任务分析失败: {:?}",
            response.text().await.unwrap_or_default()
        );
    }

    // 3.2 性能预测
    println!("\n3.2 预测记忆配置性能...");
    let req = PredictRequest {
        memory_config: serde_json::json!({
            "stmWeight": 0.7,
            "ltmWeight": 0.3,
            "kgWeight": 0.0,
            "mmWeight": 0.0,
        }),
        task_profile: TaskProfile {
            task_type: "knowledge_query".to_string(),
            complexity: "medium".to_string(),
            reasoning_depth: "deep".to_string(),
            domain: Some("技术".to_string()),
        },
    };

    let response = ctx
        .client
        .post(format!(
            "{}/api/v1/memory/predictor/performance",
            ctx.base_url
        ))
        .json(&req)
        .send()
        .await?;

    if response.status().is_success() {
        let result: PredictResponse = response.json().await?;
        println!("✓ 性能预测结果:");
        println!("  - 效率提升: {:.2}%", result.efficiency_gain * 100.0);
        println!("  - 连贯性提升: {:.2}%", result.coherence_gain * 100.0);
        println!("  - 资源成本: {:.2}", result.resource_cost);
    } else {
        println!(
            "✗ 性能预测失败: {:?}",
            response.text().await.unwrap_or_default()
        );
    }

    Ok(())
}

async fn demo_status(ctx: &DemoContext) -> Result<()> {
    println!("\n{}", "=".repeat(60));
    println!("演示4: 记忆系统状态");
    println!("{}\n", "=".repeat(60));

    // 4.1 获取资源状态
    println!("4.1 获取系统资源状态...");
    let response = ctx
        .client
        .get(format!("{}/api/v1/memory/monitor/resources", ctx.base_url))
        .send()
        .await?;

    if response.status().is_success() {
        let result: ResourcesResponse = response.json().await?;
        println!("✓ 系统资源状态:");
        println!("  - CPU使用率: {:.1}%", result.cpu);
        println!("  - 内存使用率: {:.1}%", result.memory);
    } else {
        println!(
            "✗ 获取资源状态失败: {:?}",
            response.text().await.unwrap_or_default()
        );
    }

    // 4.2 获取健康状态
    println!("\n4.2 获取健康状态...");
    let response = ctx
        .client
        .get(format!("{}/api/v1/memory/health", ctx.base_url))
        .send()
        .await?;

    if response.status().is_success() {
        let text = response.text().await?;
        println!("✓ 健康检查: {}", text);
    } else {
        println!(
            "✗ 健康检查失败: {:?}",
            response.text().await.unwrap_or_default()
        );
    }

    Ok(())
}

// ============================================================================
// 主函数
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    println!("\n{}", "#".to_string().repeat(60));
    println!("# 自适应记忆系统 - 完整功能演示");
    println!("{}", "#".to_string().repeat(60));

    let ctx = DemoContext::new("http://localhost:8008");

    println!("\n演示说明:");
    println!("1. 确保API服务已启动: cd backend && cargo run");
    println!("2. 服务地址: http://localhost:8008");
    println!("3. 注意: 向量搜索需要Qdrant服务运行在localhost:6334");
    println!("4. 按Enter键开始演示...");

    std::io::stdin().read_line(&mut String::new())?;

    // 执行所有演示
    demo_stm(&ctx).await?;
    demo_ltm(&ctx).await?;
    demo_analysis(&ctx).await?;
    demo_status(&ctx).await?;

    println!("\n{}", "#".to_string().repeat(60));
    println!("# 演示完成!");
    println!("{}", "#".to_string().repeat(60));
    println!("\n更多API端点请参考: docs/MEMORY_API_EXAMPLES.md");

    Ok(())
}
