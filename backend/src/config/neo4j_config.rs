use serde::Deserialize;

/// Neo4j配置
#[derive(Deserialize, Clone, Debug)]
pub struct Neo4jConfig {
    #[serde(default = "default_neo4j_host")]
    pub host: String,
    #[serde(default = "default_neo4j_port")]
    pub port: u16,
    #[serde(default = "default_neo4j_username")]
    pub username: String,
    #[serde(default = "default_neo4j_password")]
    pub password: String,
    #[serde(default = "default_neo4j_database")]
    pub database: String,
}

fn default_neo4j_host() -> String {
    "localhost".to_string()
}

fn default_neo4j_port() -> u16 {
    7687  // Neo4j Bolt 协议默认端口
}

fn default_neo4j_username() -> String {
    "neo4j".to_string()
}

fn default_neo4j_password() -> String {
    "password".to_string()
}

fn default_neo4j_database() -> String {
    "neo4j".to_string()
}
