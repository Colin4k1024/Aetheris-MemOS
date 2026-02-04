use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_host")]
    pub host: String,
    #[serde(default = "default_qdrant_port")]
    pub port: u16,
    #[serde(default = "default_collection_name")]
    pub collection_name: String,
    #[serde(default = "default_vector_dimension")]
    pub vector_dimension: usize,
    #[serde(default = "default_distance_type")]
    pub distance_type: String,
}

fn default_qdrant_host() -> String {
    "localhost".to_string()
}

fn default_qdrant_port() -> u16 {
    6334  // gRPC 端口（qdrant-client 使用 gRPC，不是 HTTP REST API）
}

fn default_collection_name() -> String {
    "long_term_memory".to_string()
}

fn default_vector_dimension() -> usize {
    768
}

fn default_distance_type() -> String {
    "Euclid".to_string()
}

