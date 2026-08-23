use a2a::agent_card::{AgentCapabilities, AgentCard, AgentInterface, AgentProvider, AgentSkill};
use a2a::types::{TRANSPORT_PROTOCOL_HTTP_JSON, TRANSPORT_PROTOCOL_JSONRPC};

pub fn create_agent_card(base_url: &str) -> AgentCard {
    let public_base_url = base_url
        .trim_end_matches('/')
        .replace("://0.0.0.0", "://127.0.0.1");

    AgentCard {
        name: "Aetheris MemOS".to_string(),
        description: "Adaptive Memory Management System for AI Agents. Provides multi-layer memory (STM/LTM/KG/MM), hybrid search, memory fusion, and self-healing capabilities.".to_string(),
        version: "1.0.0".to_string(),
        provider: Some(AgentProvider {
            organization: "Aetheris".to_string(),
            url: "https://github.com/Aetheris-MemOS".to_string(),
        }),
        capabilities: AgentCapabilities {
            // P2: SSE streaming sends working→completed events, not true incremental
            // results. Mark as false until real incremental streaming is implemented.
            streaming: Some(false),
            push_notifications: Some(false),
            extensions: None,
            extended_agent_card: None,
        },
        skills: vec![
            AgentSkill {
                id: "memory_search".to_string(),
                name: "Memory Search".to_string(),
                description: "Search across all memory layers (STM, LTM, KG, MM) with hybrid retrieval".to_string(),
                tags: vec!["memory".to_string(), "search".to_string(), "hybrid".to_string()],
                examples: Some(vec![
                    "Search for memories about machine learning".to_string(),
                    "Find recent conversations about project planning".to_string(),
                ]),
                input_modes: Some(vec!["text/plain".to_string(), "application/json".to_string()]),
                output_modes: Some(vec!["application/json".to_string()]),
                security_requirements: None,
            },
            AgentSkill {
                id: "memory_store".to_string(),
                name: "Memory Store".to_string(),
                description: "Store a conversation message in short-term memory".to_string(),
                tags: vec!["memory".to_string(), "store".to_string(), "stm".to_string()],
                examples: Some(vec![
                    "Remember this conversation context".to_string(),
                ]),
                input_modes: Some(vec!["text/plain".to_string(), "application/json".to_string()]),
                output_modes: Some(vec!["application/json".to_string()]),
                security_requirements: None,
            },
            AgentSkill {
                id: "memory_fusion".to_string(),
                name: "Memory Fusion".to_string(),
                description: "Query across all memory layers with unified fusion results".to_string(),
                tags: vec!["memory".to_string(), "fusion".to_string(), "multi-layer".to_string()],
                examples: Some(vec![
                    "Search everything about this topic".to_string(),
                    "Get comprehensive context from all memory layers".to_string(),
                ]),
                input_modes: Some(vec!["text/plain".to_string(), "application/json".to_string()]),
                output_modes: Some(vec!["application/json".to_string()]),
                security_requirements: None,
            },
            AgentSkill {
                id: "knowledge_graph".to_string(),
                name: "Knowledge Graph".to_string(),
                description: "Query knowledge graph entities and related knowledge".to_string(),
                tags: vec!["knowledge".to_string(), "graph".to_string(), "entities".to_string()],
                examples: Some(vec![
                    "Find entities related to this concept".to_string(),
                    "Get knowledge graph connections".to_string(),
                ]),
                input_modes: Some(vec!["text/plain".to_string(), "application/json".to_string()]),
                output_modes: Some(vec!["application/json".to_string()]),
                security_requirements: None,
            },
        ],
        default_input_modes: vec!["text/plain".to_string(), "application/json".to_string()],
        default_output_modes: vec!["application/json".to_string()],
        supported_interfaces: vec![
            AgentInterface {
                url: format!("{}/a2a/jsonrpc", public_base_url),
                protocol_binding: TRANSPORT_PROTOCOL_JSONRPC.to_string(),
                protocol_version: "1.0.0".to_string(),
                tenant: None,
            },
            AgentInterface {
                url: format!("{}/a2a/rest/messages", public_base_url),
                protocol_binding: TRANSPORT_PROTOCOL_HTTP_JSON.to_string(),
                protocol_version: "1.0.0".to_string(),
                tenant: None,
            },
        ],
        security_schemes: None,
        security_requirements: None,
        documentation_url: Some("https://github.com/Aetheris-MemOS".to_string()),
        icon_url: None,
        signatures: None,
    }
}
