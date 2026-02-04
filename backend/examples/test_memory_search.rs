/// 记忆系统集成测试
/// 
/// 本测试脚本用于验证记忆存储和搜索功能：
/// 1. 写入10条不同主题的记忆数据
/// 2. 使用不同的查询词进行搜索测试
/// 3. 验证搜索结果是否正确
/// 
/// 运行方式：
///   1. 确保服务已启动: cargo run
///   2. 运行测试: cargo run --example test_memory_search

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{error, info, warn};

/// 测试记忆数据结构
#[derive(Debug)]
struct TestMemory {
    source_id: String,
    source_type: String,
    content: String,
    title: Option<String>,
    #[allow(dead_code)] // 保留用于未来扩展
    expected_keywords: Vec<String>,
}

/// 存储 LTM 请求
#[derive(Serialize)]
struct StoreLTMRequest {
    #[serde(rename = "sourceId")]
    source_id: String,
    #[serde(rename = "sourceType")]
    source_type: String,
    content: String,
    title: Option<String>,
}

/// 存储 LTM 响应
#[derive(Deserialize)]
struct StoreLTMResponse {
    #[serde(rename = "entryId")]
    entry_id: String,
}

/// 搜索 LTM 请求
#[derive(Serialize)]
struct SearchLTMRequest {
    query: String,
    #[serde(rename = "topK")]
    top_k: Option<usize>,
    #[serde(rename = "enableRerank")]
    enable_rerank: Option<bool>,
    #[serde(rename = "minScore")]
    min_score: Option<f32>,
}

/// 搜索结果
#[derive(Deserialize, Debug)]
struct SearchResult {
    #[serde(rename = "entry_id")]
    entry_id: String,
    score: f32,
    content: String,
    title: Option<String>,
    #[allow(dead_code)] // 从 API 响应中反序列化，保留用于未来扩展
    metadata: serde_json::Value,
}

/// 搜索 LTM 响应
#[derive(Deserialize)]
struct SearchLTMResponse {
    results: Vec<SearchResult>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt::init();
    
    info!("=== 记忆系统集成测试开始 ===");
    
    // API 基础 URL
    let base_url = std::env::var("API_BASE_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8008".to_string());
    
    info!("API 基础 URL: {}", base_url);
    
    // 创建 HTTP 客户端
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;
    
    // 检查服务是否可用
    if !check_service_available(&client, &base_url).await {
        error!("服务不可用，请确保服务已启动: cargo run");
        return Err(anyhow::anyhow!("服务不可用"));
    }
    
    // 检查依赖服务
    let deps_ok = check_dependencies().await;
    if !deps_ok {
        warn!("\n⚠ 警告：部分依赖服务不可用");
        warn!("测试可能会失败，但将继续尝试...\n");
    }
    
    // 准备测试数据
    let test_memories = prepare_test_memories();
    info!("准备了 {} 条测试记忆数据", test_memories.len());
    
    // 执行测试
    match run_tests(&client, &base_url, test_memories).await {
        Ok(()) => {
            info!("=== 所有测试通过 ===");
            Ok(())
        }
        Err(e) => {
            error!("测试失败: {}", e);
            Err(e)
        }
    }
}

/// 准备10条测试记忆数据
fn prepare_test_memories() -> Vec<TestMemory> {
    vec![
        TestMemory {
            source_id: "test_rust_1".to_string(),
            source_type: "test".to_string(),
            content: "Rust 是一种系统编程语言，由 Mozilla 开发。它注重内存安全、并发安全和性能。Rust 通过所有权系统在编译时防止内存错误，无需垃圾回收器就能保证内存安全。Rust 还支持零成本抽象，让开发者可以编写高级代码而不损失性能。".to_string(),
            title: Some("Rust 编程语言介绍".to_string()),
            expected_keywords: vec!["Rust".to_string(), "系统编程".to_string(), "内存安全".to_string()],
        },
        TestMemory {
            source_id: "test_python_1".to_string(),
            source_type: "test".to_string(),
            content: "Python 是一种高级编程语言，由 Guido van Rossum 创建。Python 语法简洁清晰，易于学习和使用。它广泛应用于数据科学、机器学习、Web 开发、自动化脚本等领域。Python 拥有丰富的标准库和第三方包生态系统。".to_string(),
            title: Some("Python 编程语言特点".to_string()),
            expected_keywords: vec!["Python".to_string(), "数据科学".to_string(), "机器学习".to_string()],
        },
        TestMemory {
            source_id: "test_database_1".to_string(),
            source_type: "test".to_string(),
            content: "数据库是用于存储和管理数据的系统。关系型数据库使用 SQL 语言进行查询，常见的有 MySQL、PostgreSQL、SQLite 等。NoSQL 数据库包括 MongoDB、Redis、Cassandra 等，适用于不同的使用场景。数据库设计需要考虑数据一致性、性能和可扩展性。".to_string(),
            title: Some("数据库基础知识".to_string()),
            expected_keywords: vec!["数据库".to_string(), "SQL".to_string(), "MySQL".to_string()],
        },
        TestMemory {
            source_id: "test_ai_1".to_string(),
            source_type: "test".to_string(),
            content: "人工智能是计算机科学的一个分支，旨在创建能够执行通常需要人类智能的任务的系统。机器学习是 AI 的一个子集，通过算法让计算机从数据中学习。深度学习使用神经网络来模拟人脑的工作方式，在图像识别、自然语言处理等领域取得了突破性进展。".to_string(),
            title: Some("人工智能概述".to_string()),
            expected_keywords: vec!["人工智能".to_string(), "机器学习".to_string(), "深度学习".to_string()],
        },
        TestMemory {
            source_id: "test_web_1".to_string(),
            source_type: "test".to_string(),
            content: "Web 开发涉及前端和后端技术。前端使用 HTML、CSS 和 JavaScript 构建用户界面。现代前端框架如 React、Vue、Angular 提供了组件化开发方式。后端负责处理业务逻辑和数据存储，可以使用 Node.js、Python、Java 等语言开发。RESTful API 是前后端通信的常用方式。".to_string(),
            title: Some("Web 开发技术栈".to_string()),
            expected_keywords: vec!["Web 开发".to_string(), "React".to_string(), "API".to_string()],
        },
        TestMemory {
            source_id: "test_cloud_1".to_string(),
            source_type: "test".to_string(),
            content: "云计算提供了通过互联网访问计算资源的方式。主要服务模式包括 IaaS（基础设施即服务）、PaaS（平台即服务）和 SaaS（软件即服务）。主流云服务提供商有 AWS、Azure、Google Cloud 等。云计算的优势包括弹性扩展、按需付费和降低运维成本。".to_string(),
            title: Some("云计算基础".to_string()),
            expected_keywords: vec!["云计算".to_string(), "AWS".to_string(), "IaaS".to_string()],
        },
        TestMemory {
            source_id: "test_security_1".to_string(),
            source_type: "test".to_string(),
            content: "网络安全是保护网络系统和数据免受攻击、损害或未授权访问的实践。常见的安全威胁包括恶意软件、网络钓鱼、DDoS 攻击等。安全措施包括使用防火墙、加密、身份验证和访问控制。定期更新软件和进行安全审计也很重要。".to_string(),
            title: Some("网络安全基础".to_string()),
            expected_keywords: vec!["网络安全".to_string(), "加密".to_string(), "防火墙".to_string()],
        },
        TestMemory {
            source_id: "test_devops_1".to_string(),
            source_type: "test".to_string(),
            content: "DevOps 是开发和运维的结合，旨在缩短软件开发生命周期。CI/CD（持续集成/持续部署）是 DevOps 的核心实践，通过自动化测试和部署提高效率。常用工具包括 Docker、Kubernetes、Jenkins、GitLab CI 等。DevOps 强调自动化、监控和协作。".to_string(),
            title: Some("DevOps 实践".to_string()),
            expected_keywords: vec!["DevOps".to_string(), "CI/CD".to_string(), "Docker".to_string()],
        },
        TestMemory {
            source_id: "test_blockchain_1".to_string(),
            source_type: "test".to_string(),
            content: "区块链是一种分布式账本技术，通过密码学方法将数据块链接在一起。每个区块包含前一个区块的哈希值，形成不可篡改的链。区块链的主要特点包括去中心化、透明性和不可篡改性。比特币和以太坊是区块链技术的典型应用。".to_string(),
            title: Some("区块链技术介绍".to_string()),
            expected_keywords: vec!["区块链".to_string(), "比特币".to_string(), "以太坊".to_string()],
        },
        TestMemory {
            source_id: "test_microservices_1".to_string(),
            source_type: "test".to_string(),
            content: "微服务架构是一种将应用程序构建为小型独立服务的方法。每个微服务专注于特定的业务功能，可以独立开发、部署和扩展。微服务之间通过 API 通信。这种架构的优势包括更好的可扩展性、技术多样性和故障隔离。但也带来了分布式系统的复杂性。".to_string(),
            title: Some("微服务架构".to_string()),
            expected_keywords: vec!["微服务".to_string(), "架构".to_string(), "API".to_string()],
        },
    ]
}

/// 检查服务是否可用
async fn check_service_available(client: &Client, base_url: &str) -> bool {
    info!("检查服务是否可用...");
    let url = format!("{}/api/v1/memory/health", base_url);
    
    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                info!("✓ 服务可用");
                true
            } else {
                warn!("服务响应异常: {}", resp.status());
                false
            }
        }
        Err(e) => {
            warn!("服务不可用: {}", e);
            false
        }
    }
}

/// 检查依赖服务是否可用
async fn check_dependencies() -> bool {
    info!("检查依赖服务...");
    let mut all_ok = true;
    
    // 检查 Ollama (LLM 服务)
    let ollama_url = "http://localhost:11434";
    info!("检查 Ollama 服务 ({})...", ollama_url);
    match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => {
            match client.get(ollama_url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!("  ✓ Ollama 服务可用");
                    } else {
                        warn!("  ⚠ Ollama 服务响应异常: {}", resp.status());
                        all_ok = false;
                    }
                }
                Err(e) => {
                    error!("  ✗ Ollama 服务不可用: {}", e);
                    error!("    请确保 Ollama 已安装并运行: https://ollama.ai/");
                    error!("    启动命令: ollama serve");
                    all_ok = false;
                }
            }
        }
        Err(e) => {
            error!("  ✗ 无法创建 HTTP 客户端: {}", e);
            all_ok = false;
        }
    }
    
    // 检查 Ollama 模型是否可用
    if all_ok {
        info!("检查 Ollama 模型...");
        let models_url = "http://localhost:11434/api/tags";
        match reqwest::get(models_url).await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                            let model_names: Vec<String> = models
                                .iter()
                                .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                                .collect();
                            info!("  已安装的模型: {:?}", model_names);
                            
                            // 检查需要的模型
                            let required_models = vec!["llama2", "nomic-embed-text"];
                            for model in required_models {
                                if model_names.iter().any(|m| m.contains(model)) {
                                    info!("  ✓ 找到模型: {}", model);
                                } else {
                                    warn!("  ⚠ 未找到模型: {} (可能需要下载)", model);
                                    warn!("    下载命令: ollama pull {}", model);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("  ⚠ 无法检查模型列表: {}", e);
            }
        }
    }
    
    // 检查 Qdrant 服务（HTTP REST API 在 6333，gRPC 在 6334）
    // qdrant-client 使用 gRPC，但我们检查 HTTP API 来确认服务是否运行
    let qdrant_http_url = "http://localhost:6333";
    info!("检查 Qdrant 服务 (HTTP API: {})...", qdrant_http_url);
    match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
    {
        Ok(client) => {
            match client.get(qdrant_http_url).send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!("  ✓ Qdrant HTTP API 可用（gRPC 端口 6334 应该也可用）");
                    } else {
                        warn!("  ⚠ Qdrant 服务响应异常: {}", resp.status());
                        all_ok = false;
                    }
                }
                Err(e) => {
                    error!("  ✗ Qdrant 服务不可用: {}", e);
                    error!("    请确保 Qdrant 已安装并运行");
                    error!("    启动命令: docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant");
                    all_ok = false;
                }
            }
        }
        Err(e) => {
            error!("  ✗ 无法创建 HTTP 客户端: {}", e);
            all_ok = false;
        }
    }
    
    if !all_ok {
        warn!("\n⚠ 部分依赖服务不可用，测试可能会失败");
        warn!("建议先启动所有依赖服务后再运行测试");
    }
    
    all_ok
}

/// 运行测试
async fn run_tests(
    client: &Client,
    base_url: &str,
    test_memories: Vec<TestMemory>,
) -> Result<()> {
    // 阶段1：写入记忆数据
    info!("\n=== 阶段1：写入记忆数据 ===");
    let mut entry_ids = HashMap::new();
    
    let store_url = format!("{}/api/v1/memory/storage/ltm", base_url);
    
    for (index, memory) in test_memories.iter().enumerate() {
        info!("写入记忆 {}/{}: {}", 
              index + 1, 
              test_memories.len(), 
              memory.title.as_deref().unwrap_or("无标题"));
        
        let request = StoreLTMRequest {
            source_id: memory.source_id.clone(),
            source_type: memory.source_type.clone(),
            content: memory.content.clone(),
            title: memory.title.clone(),
        };
        
        match client
            .post(&store_url)
            .json(&request)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let url = resp.url().clone();
                // 在消耗 resp 之前先获取所有需要的信息
                let content_type = resp.headers()
                    .get("content-type")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string();
                
                if status.is_success() {
                    info!("  响应Content-Type: {}", content_type);
                    
                    // 使用 bytes() 先获取原始字节，然后转换为字符串
                    let response_text = match resp.bytes().await {
                        Ok(bytes) => {
                            if bytes.is_empty() {
                                error!("  ✗ 响应体为空");
                                warn!("  跳过此条记录，继续测试...");
                                continue;
                            }
                            match String::from_utf8(bytes.to_vec()) {
                                Ok(text) => text,
                                Err(e) => {
                                    error!("  ✗ 响应体不是有效的UTF-8: {}", e);
                                    error!("  响应字节长度: {}", bytes.len());
                                    warn!("  跳过此条记录，继续测试...");
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            error!("  ✗ 读取响应失败: {}", e);
                            error!("  状态码: {}, URL: {}", status, url);
                            warn!("  跳过此条记录，继续测试...");
                            continue;
                        }
                    };
                    
                    info!("  响应内容 (前200字符): {}", response_text.chars().take(200).collect::<String>());
                    
                    match serde_json::from_str::<StoreLTMResponse>(&response_text) {
                        Ok(data) => {
                            info!("  ✓ 成功写入，entry_id: {}", data.entry_id);
                            entry_ids.insert(memory.source_id.clone(), data.entry_id);
                        }
                        Err(e) => {
                            error!("  ✗ 解析响应失败: {}", e);
                            error!("  状态码: {}", status);
                            error!("  Content-Type: {}", content_type);
                            if response_text.len() > 500 {
                                error!("  响应前500字符: {}", response_text.chars().take(500).collect::<String>());
                            } else {
                                error!("  完整响应: {}", response_text);
                            }
                            warn!("  跳过此条记录，继续测试...");
                            continue; // 继续处理下一条记录
                        }
                    }
                } else {
                    let status = resp.status();
                    let error_text = resp.text().await.unwrap_or_default();
                    
                    // 尝试提取 JSON 错误信息（如果有）
                    let error_msg = if error_text.starts_with('{') {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                            json.get("message")
                                .or_else(|| json.get("error"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(&error_text)
                                .to_string()
                        } else {
                            error_text
                        }
                    } else {
                        // HTML 错误页面，提取关键信息
                        if error_text.contains("Internal Server Error") {
                            "内部服务器错误 - 可能是依赖服务（Ollama/Qdrant）不可用".to_string()
                        } else {
                            error_text.chars().take(200).collect()
                        }
                    };
                    
                    error!("  ✗ 写入失败: {} - {}", status, error_msg);
                    warn!("  跳过此条记录，继续测试...");
                    
                    // 如果是第一个请求失败，提供诊断建议
                    if index == 0 {
                        error!("\n诊断建议：");
                        error!("1. 检查 Ollama 服务是否运行: curl http://localhost:11434");
                        error!("2. 检查 Qdrant 服务是否运行: curl http://localhost:6333 (HTTP API)");
                        error!("   注意：qdrant-client 使用 gRPC 端口 6334，确保 Qdrant 同时暴露 6333 和 6334 端口");
                        error!("3. 查看后端服务日志获取详细错误信息");
                        error!("4. 确保所有依赖服务已正确配置");
                    }
                    
                    continue; // 继续处理下一条记录，而不是退出
                }
            }
            Err(e) => {
                error!("  ✗ 请求失败: {}", e);
                warn!("  跳过此条记录，继续测试...");
                continue; // 继续处理下一条记录
            }
        }
        
        // 添加短暂延迟，避免请求过快
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    
    let success_count = entry_ids.len();
    let total_count = test_memories.len();
    
    if success_count == 0 {
        error!("所有写入都失败了，无法继续搜索测试");
        return Err(anyhow::anyhow!("所有写入都失败了"));
    }
    
    if success_count < total_count {
        warn!("成功写入 {} 条记忆数据（共 {} 条，{} 条失败）", 
              success_count, total_count, total_count - success_count);
    } else {
        info!("成功写入 {} 条记忆数据", success_count);
    }
    
    // 等待一下，确保数据已完全处理（LLM 总结和向量化需要时间）
    info!("等待数据处理完成（LLM 总结和向量化）...");
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    // 阶段2：搜索测试
    info!("\n=== 阶段2：搜索测试 ===");
    
    // 测试不同的查询词
    let test_queries = vec![
        ("Rust", vec!["test_rust_1"]),
        ("Python", vec!["test_python_1"]),
        ("数据库", vec!["test_database_1"]),
        ("机器学习", vec!["test_ai_1"]),
        ("Web 开发", vec!["test_web_1"]),
        ("云计算", vec!["test_cloud_1"]),
        ("网络安全", vec!["test_security_1"]),
        ("DevOps", vec!["test_devops_1"]),
        ("区块链", vec!["test_blockchain_1"]),
        ("微服务", vec!["test_microservices_1"]),
    ];
    
    let search_url = format!("{}/api/v1/memory/search/ltm", base_url);
    let mut passed_tests = 0;
    let mut failed_tests = 0;
    
    for (query, expected_source_ids) in test_queries {
        info!("\n测试查询: \"{}\"", query);
        
        let search_request = SearchLTMRequest {
            query: query.to_string(),
            top_k: Some(10),
            enable_rerank: Some(false), // 禁用以加快测试
            min_score: None,
        };
        
        match client
            .post(&search_url)
            .json(&search_request)
            .send()
            .await
        {
            Ok(resp) => {
                let status = resp.status();
                let url = resp.url().clone();
                let content_type = resp.headers()
                    .get("content-type")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("unknown")
                    .to_string();
                
                if status.is_success() {
                    info!("  响应Content-Type: {}", content_type);
                    
                    // 使用 bytes() 先获取原始字节，然后转换为字符串
                    let response_text = match resp.bytes().await {
                        Ok(bytes) => {
                            if bytes.is_empty() {
                                error!("  ✗ 响应体为空");
                                failed_tests += 1;
                                continue;
                            }
                            match String::from_utf8(bytes.to_vec()) {
                                Ok(text) => text,
                                Err(e) => {
                                    error!("  ✗ 响应体不是有效的UTF-8: {}", e);
                                    error!("  响应字节长度: {}", bytes.len());
                                    failed_tests += 1;
                                    continue;
                                }
                            }
                        }
                        Err(e) => {
                            error!("  ✗ 读取响应失败: {}", e);
                            error!("  状态码: {}, URL: {}", status, url);
                            failed_tests += 1;
                            continue;
                        }
                    };
                    
                    info!("  响应内容 (前200字符): {}", response_text.chars().take(200).collect::<String>());
                    
                    match serde_json::from_str::<SearchLTMResponse>(&response_text) {
                        Ok(data) => {
                            let results = data.results;
                            info!("  找到 {} 条结果", results.len());
                            
                            if results.is_empty() {
                                warn!("  ✗ 未找到任何结果");
                                failed_tests += 1;
                                continue;
                            }
                            
                            // 验证结果
                            let mut found_expected = false;
                            let expected_source_id = expected_source_ids[0].to_string();
                            for result in &results {
                                // 检查是否包含预期的记忆
                                if let Some(expected_entry_id) = entry_ids.get(&expected_source_id) {
                                    if result.entry_id == *expected_entry_id {
                                        found_expected = true;
                                        info!("  ✓ 找到预期结果: entry_id={}, score={:.4}", 
                                              result.entry_id, result.score);
                                        break;
                                    }
                                }
                            }
                            
                            if found_expected {
                                passed_tests += 1;
                                info!("  ✓ 测试通过");
                            } else {
                                warn!("  ✗ 未找到预期的记忆条目");
                                info!("  预期 source_id: {:?}", expected_source_ids);
                                info!("  实际找到的 entry_ids: {:?}", 
                                      results.iter().map(|r| &r.entry_id).collect::<Vec<_>>());
                                failed_tests += 1;
                            }
                            
                            // 显示前3个结果
                            for result in results.iter().take(3) {
                                let title = result.title.as_ref()
                                    .map(|t| t.as_str())
                                    .unwrap_or("无标题");
                                let preview: String = result.content.chars().take(50).collect();
                                info!("    [{:.4}] {} - {}", 
                                      result.score, title, preview);
                            }
                        }
                        Err(e) => {
                            error!("  ✗ 解析响应失败: {}", e);
                            error!("  状态码: {}", status);
                            error!("  Content-Type: {}", content_type);
                            if response_text.len() > 500 {
                                error!("  响应前500字符: {}", response_text.chars().take(500).collect::<String>());
                            } else {
                                error!("  完整响应: {}", response_text);
                            }
                            failed_tests += 1;
                        }
                    }
                } else {
                    let status = resp.status();
                    let error_text = resp.text().await.unwrap_or_default();
                    error!("  ✗ 搜索失败: {} - {}", status, error_text);
                    failed_tests += 1;
                }
            }
            Err(e) => {
                error!("  ✗ 请求失败: {}", e);
                failed_tests += 1;
            }
        }
    }
    
    // 测试总结
    let total_tests = passed_tests + failed_tests;
    info!("\n=== 测试总结 ===");
    info!("总测试数: {}", total_tests);
    info!("通过: {}", passed_tests);
    info!("失败: {}", failed_tests);
    
    if failed_tests > 0 {
        Err(anyhow::anyhow!("有 {} 个测试失败", failed_tests))
    } else {
        Ok(())
    }
}
