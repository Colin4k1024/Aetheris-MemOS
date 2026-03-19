use anyhow::Result;
use qdrant_client::qdrant::{
    CreateCollection, Distance, PointId, PointStruct, SearchPoints, VectorParams, Vectors,
    VectorsConfig,
};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, instrument, warn};

use crate::config;

/// Qdrant 客户端，用于向量数据库操作
pub struct QdrantClient {
    client: qdrant_client::Qdrant,
    collection_name: String,
    vector_dimension: usize,
    distance: Distance,
}

/// 搜索结果（与原有接口保持兼容）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    /// 向量 ID
    pub id: String,
    /// 相似度分数（距离）
    pub score: f32,
    /// 元数据
    pub metadata: Value,
}

impl QdrantClient {
    /// 创建新的 Qdrant 客户端实例
    pub fn new() -> Result<Self> {
        let config = config::get();
        // qdrant-client 使用 gRPC，URL 格式应该是 http://host:port
        // 注意：虽然使用 http:// 前缀，但实际通信使用 gRPC (HTTP/2)
        let url = format!("http://{}:{}", config.qdrant.host, config.qdrant.port);

        let client = qdrant_client::Qdrant::from_url(&url).build().map_err(|e| {
            error!("Failed to create Qdrant client: {}", e);
            anyhow::anyhow!("Failed to create Qdrant client: {}", e)
        })?;

        // 将距离类型字符串转换为 Distance 枚举
        let distance = match config.qdrant.distance_type.as_str() {
            "Cosine" => Distance::Cosine,
            "Dot" => Distance::Dot,
            "Euclid" | "L2" => Distance::Euclid,
            _ => {
                warn!(
                    "Unknown distance type: {}, using Euclid",
                    config.qdrant.distance_type
                );
                Distance::Euclid
            }
        };

        info!(
            "Qdrant client initialized: url={}, collection={}, dimension={}, distance={:?}",
            url, config.qdrant.collection_name, config.qdrant.vector_dimension, distance
        );

        Ok(Self {
            client,
            collection_name: config.qdrant.collection_name.clone(),
            vector_dimension: config.qdrant.vector_dimension,
            distance,
        })
    }

    /// 确保集合存在，如果不存在则创建
    #[instrument(skip(self))]
    pub async fn ensure_collection(&self) -> Result<()> {
        info!("Ensuring collection exists: {}", self.collection_name);

        // Issue #59: initialise the vector-space guard (signature check).
        // This is a no-op (cheap OnceLock check) after the first call.
        if let Err(e) = crate::services::vector_guard::init() {
            // Propagate dimension-mismatch as a hard error; model-change warnings
            // are already logged inside init() and do not return Err.
            return Err(e);
        }

        // 检查集合是否存在
        if self.collection_exists().await? {
            info!("Collection already exists: {}", self.collection_name);
            return Ok(());
        }

        // 创建集合
        self.create_collection().await?;
        info!("Collection created: {}", self.collection_name);

        Ok(())
    }

    /// 检查集合是否存在
    async fn collection_exists(&self) -> Result<bool> {
        match self.client.list_collections().await {
            Ok(collections) => {
                let exists = collections
                    .collections
                    .iter()
                    .any(|c| c.name == self.collection_name);
                Ok(exists)
            }
            Err(e) => {
                error!("Failed to list collections: {}", e);
                Err(anyhow::anyhow!("Failed to list collections: {}", e))
            }
        }
    }

    /// 创建集合
    async fn create_collection(&self) -> Result<()> {
        use qdrant_client::qdrant::vectors_config::Config;

        self.client
            .create_collection(CreateCollection {
                collection_name: self.collection_name.clone(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: self.vector_dimension as u64,
                        distance: self.distance as i32,
                        datatype: Some(qdrant_client::qdrant::Datatype::Float32 as i32),
                        hnsw_config: None,
                        multivector_config: None,
                        quantization_config: None,
                        on_disk: None,
                    })),
                }),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                error!("Failed to create collection: {}", e);
                anyhow::anyhow!("Failed to create collection: {}", e)
            })?;

        Ok(())
    }

    /// 插入向量
    #[instrument(skip(self))]
    pub async fn insert_vectors(
        &self,
        vectors: Vec<Vec<f32>>,
        ids: Vec<String>,
        metadata: Vec<Value>,
    ) -> Result<()> {
        let vector_count = vectors.len();
        info!("Inserting {} vectors into collection", vector_count);

        // Issue #59: validate that every vector has the expected dimension.
        crate::services::vector_guard::validate_write(&vectors)?;

        // 确保集合存在
        self.ensure_collection().await?;

        // 将向量、ID 和元数据组合成 PointStruct
        let points: Vec<PointStruct> = vectors
            .into_iter()
            .zip(ids.into_iter())
            .zip(metadata.into_iter())
            .map(|((vector, id), meta)| {
                // 将 ULID 字符串转换为整数 ID（使用字符串哈希）
                // Qdrant 的 UUID 字段要求标准 UUID 格式，而 ULID 不是标准 UUID
                // 因此我们使用整数 ID，并将原始 ULID 存储在 payload 中
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                id.hash(&mut hasher);
                let numeric_id = hasher.finish();

                // 将原始 ULID 添加到 payload 中
                let mut payload = json_to_qdrant_payload(&meta);
                payload.insert(
                    "_original_id".to_string(),
                    qdrant_client::qdrant::Value {
                        kind: Some(qdrant_client::qdrant::value::Kind::StringValue(id)),
                    },
                );

                use qdrant_client::qdrant::point_id::PointIdOptions;
                use qdrant_client::qdrant::vectors::VectorsOptions;

                let point_id = PointId {
                    point_id_options: Some(PointIdOptions::Num(numeric_id)),
                };

                PointStruct::new(
                    point_id,
                    Vectors {
                        vectors_options: Some(VectorsOptions::Vector(
                            qdrant_client::qdrant::Vector::new(vector.clone()),
                        )),
                    },
                    payload,
                )
            })
            .collect();

        use qdrant_client::qdrant::UpsertPoints;

        self.client
            .upsert_points(UpsertPoints {
                collection_name: self.collection_name.clone(),
                points,
                wait: None,
                ordering: None,
                shard_key_selector: None,
                update_filter: None,
                timeout: None,
                update_mode: None,
            })
            .await
            .map_err(|e| {
                error!("Failed to insert vectors: {}", e);
                anyhow::anyhow!("Failed to insert vectors: {}", e)
            })?;

        info!("Successfully inserted {} vectors", vector_count);
        Ok(())
    }

    /// 向量相似度搜索
    #[instrument(skip(self))]
    pub async fn search(&self, query_vector: Vec<f32>, top_k: usize) -> Result<Vec<SearchResult>> {
        self.search_with_filter(query_vector, top_k, None).await
    }

    /// 向量相似度搜索（带过滤器）
    #[instrument(skip(self))]
    pub async fn search_with_filter(
        &self,
        query_vector: Vec<f32>,
        top_k: usize,
        filter: Option<qdrant_client::qdrant::Filter>,
    ) -> Result<Vec<SearchResult>> {
        info!(
            "Searching for similar vectors, top_k={}, has_filter={}",
            top_k,
            filter.is_some()
        );

        // Issue #59: validate query vector dimension before sending to Qdrant.
        crate::services::vector_guard::validate_read(&query_vector)?;

        // 构建搜索请求
        let search_points = SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_vector,
            limit: top_k as u64,
            with_payload: Some(true.into()),
            with_vectors: Some(false.into()),
            filter,
            score_threshold: None,
            params: None,
            offset: None,
            vector_name: None,
            ..Default::default()
        };

        let search_result = self
            .client
            .search_points(search_points)
            .await
            .map_err(|e| {
                error!("Failed to search vectors: {}", e);
                anyhow::anyhow!("Failed to search vectors: {}", e)
            })?;

        // 转换搜索结果
        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .map(|point| {
                // 将 Qdrant Payload 转换回 JSON Value
                let metadata = qdrant_payload_to_json(&point.payload);

                // 提取 ID
                // 优先从 payload 中获取原始 ULID，如果没有则使用 PointId
                let id = if let Some(original_id_value) = point.payload.get("_original_id") {
                    if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &original_id_value.kind {
                        s.clone()
                    } else {
                        match point.id {
                            Some(PointId {
                                point_id_options: Some(
                                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid),
                                ),
                            }) => uuid,
                            Some(PointId {
                                point_id_options: Some(
                                    qdrant_client::qdrant::point_id::PointIdOptions::Num(num),
                                ),
                            }) => num.to_string(),
                            _ => "".to_string(),
                        }
                    }
                } else {
                    match point.id {
                        Some(PointId {
                            point_id_options: Some(
                                qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid),
                            ),
                        }) => uuid,
                        Some(PointId {
                            point_id_options: Some(
                                qdrant_client::qdrant::point_id::PointIdOptions::Num(num),
                            ),
                        }) => num.to_string(),
                        _ => "".to_string(),
                    }
                };

                SearchResult {
                    id,
                    score: point.score,
                    metadata,
                }
            })
            .collect();

        info!("Search completed, found {} results", results.len());
        Ok(results)
    }

    /// 删除向量
    #[instrument(skip(self))]
    pub async fn delete_vectors(&self, ids: Vec<String>) -> Result<()> {
        info!("Deleting {} vectors from collection", ids.len());

        use qdrant_client::qdrant::point_id::PointIdOptions;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // 将字符串 ID（ULID）转换为整数 ID（使用哈希）
        let point_ids: Vec<PointId> = ids
            .into_iter()
            .map(|id| {
                let mut hasher = DefaultHasher::new();
                id.hash(&mut hasher);
                let numeric_id = hasher.finish();
                PointId {
                    point_id_options: Some(PointIdOptions::Num(numeric_id)),
                }
            })
            .collect();

        use qdrant_client::qdrant::points_selector::PointsSelectorOneOf;

        use qdrant_client::qdrant::DeletePoints;

        self.client
            .delete_points(DeletePoints {
                collection_name: self.collection_name.clone(),
                wait: None,
                ordering: None,
                points: Some(qdrant_client::qdrant::PointsSelector {
                    points_selector_one_of: Some(PointsSelectorOneOf::Points(
                        qdrant_client::qdrant::PointsIdsList { ids: point_ids },
                    )),
                }),
                shard_key_selector: None,
                timeout: None,
            })
            .await
            .map_err(|e| {
                error!("Failed to delete vectors: {}", e);
                anyhow::anyhow!("Failed to delete vectors: {}", e)
            })?;

        info!("Successfully deleted vectors");
        Ok(())
    }
}

/// 将 JSON Value 转换为 Qdrant Payload
fn json_to_qdrant_payload(json: &Value) -> HashMap<String, qdrant_client::qdrant::Value> {
    let mut payload = HashMap::new();
    if let Some(obj) = json.as_object() {
        for (k, v) in obj {
            payload.insert(k.clone(), json_value_to_qdrant_value(v));
        }
    }
    payload
}

/// 将 JSON Value 转换为 Qdrant Value
fn json_value_to_qdrant_value(v: &Value) -> qdrant_client::qdrant::Value {
    match v {
        Value::String(s) => qdrant_client::qdrant::Value {
            kind: Some(qdrant_client::qdrant::value::Kind::StringValue(s.clone())),
        },
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)),
                }
            } else if let Some(f) = n.as_f64() {
                qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::DoubleValue(f)),
                }
            } else {
                qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                        n.to_string(),
                    )),
                }
            }
        }
        Value::Bool(b) => qdrant_client::qdrant::Value {
            kind: Some(qdrant_client::qdrant::value::Kind::BoolValue(*b)),
        },
        Value::Null => qdrant_client::qdrant::Value {
            kind: Some(qdrant_client::qdrant::value::Kind::NullValue(1)),
        },
        Value::Array(_) | Value::Object(_) => {
            // 对于复杂类型，转换为 JSON 字符串
            qdrant_client::qdrant::Value {
                kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                    serde_json::to_string(v).unwrap_or_default(),
                )),
            }
        }
    }
}

/// 将 Qdrant Payload 转换回 JSON Value
fn qdrant_payload_to_json(payload: &HashMap<String, qdrant_client::qdrant::Value>) -> Value {
    let mut map = serde_json::Map::new();
    for (k, v) in payload {
        map.insert(k.clone(), qdrant_value_to_json_value(v));
    }
    Value::Object(map)
}

/// 将 Qdrant Value 转换回 JSON Value
fn qdrant_value_to_json_value(v: &qdrant_client::qdrant::Value) -> Value {
    match &v.kind {
        Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Value::String(s.clone()),
        Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)) => Value::Number((*i).into()),
        Some(qdrant_client::qdrant::value::Kind::DoubleValue(f)) => Value::Number(
            serde_json::Number::from_f64(*f).unwrap_or_else(|| serde_json::Number::from(0)),
        ),
        Some(qdrant_client::qdrant::value::Kind::BoolValue(b)) => Value::Bool(*b),
        Some(qdrant_client::qdrant::value::Kind::NullValue(_)) => Value::Null,
        _ => Value::Null,
    }
}

/// 全局 Qdrant 客户端实例
static QDRANT_CLIENT: once_cell::sync::OnceCell<QdrantClient> = once_cell::sync::OnceCell::new();

/// 获取全局 Qdrant 客户端实例
pub fn get_qdrant_client() -> Result<&'static QdrantClient> {
    QDRANT_CLIENT
        .get_or_try_init(|| QdrantClient::new())
        .map_err(|e| anyhow::anyhow!("Failed to initialize Qdrant client: {}", e))
}
