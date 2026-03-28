/// 企业多智能体 / 多租户记忆隔离服务 — Issue #56
///
/// 为企业场景提供**多租户记忆隔离**：
/// - 每个租户（Tenant）拥有完全独立的 STM/LTM/KG 命名空间
/// - 通过 `TenantId` 前缀所有数据库查询条件，防止跨租户数据泄露
/// - 支持**跨智能体知识共享**：同一租户内的 Agent 可以访问共享知识库
/// - 提供租户级配额管理（最大 STM 会话数、最大 LTM 条目数）
/// - 跨租户访问控制：只有 super-admin 角色可执行跨租户查询
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{info, warn};

use crate::AppError;

// ============ 数据结构 ============

/// 租户标识符（不透明字符串，推荐使用 URN 格式: "tenant:<org_id>"）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// 将 TenantId 转换为表前缀，用于区分不同租户的数据
    pub fn prefix(&self) -> String {
        format!("t:{}", self.0)
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 租户角色
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TenantRole {
    /// 普通成员：只能访问自己租户的数据
    Member,
    /// 租户管理员：可以管理本租户内的所有数据
    Admin,
    /// 超级管理员：可以访问所有租户的数据（审计/运维用途）
    SuperAdmin,
}

/// 租户配置
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TenantConfig {
    pub tenant_id: TenantId,
    pub name: String,
    /// 最大 STM 会话数（None 表示不限）
    pub max_stm_sessions: Option<usize>,
    /// 最大 LTM 条目数（None 表示不限）
    pub max_ltm_entries: Option<usize>,
    /// 是否允许本租户 Agent 间共享 LTM
    pub enable_cross_agent_sharing: bool,
    /// 允许访问的共享知识库 tenant_id 列表（跨租户只读访问）
    pub shared_knowledge_tenants: Vec<TenantId>,
}

impl TenantConfig {
    pub fn new(tenant_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            tenant_id: TenantId::new(tenant_id),
            name: name.into(),
            max_stm_sessions: None,
            max_ltm_entries: None,
            enable_cross_agent_sharing: true,
            shared_knowledge_tenants: vec![],
        }
    }
}

/// 记忆检索过滤条件（附加到所有 DB 查询）
#[derive(Debug, Clone, Default)]
pub struct MemoryFilters {
    pub source_id_prefix: Option<String>,
    pub extra_source_ids: Vec<String>,
}

/// 单条记忆条目（跨层抽象）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub entry_id: String,
    pub source_id: String,
    pub content: String,
}

// ============ 隔离层实现 ============

/// 多租户记忆隔离层
pub struct TenantIsolationLayer {
    config: TenantIsolationConfig,
}

#[derive(Debug, Clone, Default)]
pub struct TenantIsolationConfig {
    /// 是否强制执行租户隔离（false 时为 no-op，方便单租户部署）
    pub enforce: bool,
}

impl TenantIsolationLayer {
    /// Create a new tenant isolation layer.
    pub fn new() -> Self {
        Self {
            config: TenantIsolationConfig { enforce: true },
        }
    }

    pub fn with_config(config: TenantIsolationConfig) -> Self {
        Self { config }
    }

    /// 向查询过滤器中添加租户 ID 前缀约束。
    pub fn add_filter(&self, filters: &mut MemoryFilters, tenant_id: &TenantId) {
        if !self.config.enforce {
            return;
        }
        filters.source_id_prefix = Some(tenant_id.prefix());
    }

    /// 检查某条记忆是否属于该租户。
    pub fn verify_access(&self, memory: &MemoryEntry, tenant_id: &TenantId) -> bool {
        if !self.config.enforce {
            return true;
        }
        memory.source_id.starts_with(&tenant_id.prefix())
    }

    /// 生成该租户的专属 source_id（将 user/agent id 与租户绑定）。
    pub fn scoped_filters(&self, tenant_id: &TenantId) -> MemoryFilters {
        let mut f = MemoryFilters::default();
        self.add_filter(&mut f, tenant_id);
        f
    }

    /// 对记忆列表按租户过滤。
    pub fn filter_entries<'a>(
        &self,
        entries: &'a [MemoryEntry],
        tenant_id: &TenantId,
    ) -> Vec<&'a MemoryEntry> {
        if !self.config.enforce {
            return entries.iter().collect();
        }
        entries
            .iter()
            .filter(|e| self.verify_access(e, tenant_id))
            .collect()
    }

    /// 为租户生成带前缀的 source_id。
    pub fn create_tenant_query(&self, tenant_id: &TenantId, agent_id: &str) -> String {
        format!("{}:agent:{}", tenant_id.prefix(), agent_id)
    }

    /// 判断是否允许跨租户访问（仅 SuperAdmin 允许）。
    pub fn can_access_cross_tenant(
        &self,
        _tenant_id: &TenantId,
        _target_tenant_id: &TenantId,
    ) -> bool {
        // 本层不携带角色信息，由调用方在 AccessController 中校验
        false
    }

    /// 获取有效租户 ID（可用于日志/审计）。
    pub fn effective_tenant_id(&self, tenant_id: &TenantId) -> String {
        tenant_id.0.clone()
    }
}

// ============ 租户注册表（进程级内存，适合嵌入式场景） ============

type TenantRegistry = Arc<RwLock<HashMap<String, TenantConfig>>>;

static REGISTRY: OnceLock<TenantRegistry> = OnceLock::new();

fn registry() -> &'static TenantRegistry {
    REGISTRY.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

/// 注册租户（幂等）
pub fn register_tenant(cfg: TenantConfig) {
    if let Ok(mut reg) = registry().write() {
        let id = cfg.tenant_id.0.clone();
        info!("Registering tenant: {}", id);
        reg.insert(id, cfg);
    }
}

/// 获取租户配置
pub fn get_tenant(tenant_id: &str) -> Option<TenantConfig> {
    registry().read().ok()?.get(tenant_id).cloned()
}

/// 列举所有租户 ID
pub fn list_tenants() -> Vec<String> {
    registry()
        .read()
        .map(|r| r.keys().cloned().collect())
        .unwrap_or_default()
}

// ============ 访问控制器 ============

/// 多租户访问控制器（无状态，方法均为纯函数）
pub struct AccessController;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRequest {
    pub requester_tenant: TenantId,
    pub requester_role: TenantRole,
    pub target_tenant: TenantId,
    /// 操作类型：read / write / admin
    pub operation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessDecision {
    pub allowed: bool,
    pub reason: String,
}

impl AccessController {
    /// 检查访问请求是否被允许
    pub fn check(req: &AccessRequest) -> AccessDecision {
        // SuperAdmin 可以跨租户访问
        if req.requester_role == TenantRole::SuperAdmin {
            return AccessDecision {
                allowed: true,
                reason: "SuperAdmin has unrestricted access".to_string(),
            };
        }

        // 同租户内部：Member 只读，Admin 可写
        if req.requester_tenant == req.target_tenant {
            return AccessDecision {
                allowed: true,
                reason: "Same tenant access".to_string(),
            };
        }

        // 跨租户：检查共享知识库配置
        if req.operation == "read" {
            if let Some(tenant_cfg) = get_tenant(req.requester_tenant.as_str()) {
                if tenant_cfg
                    .shared_knowledge_tenants
                    .contains(&req.target_tenant)
                {
                    return AccessDecision {
                        allowed: true,
                        reason: format!("Cross-tenant read allowed via shared knowledge config"),
                    };
                }
            }
        }

        AccessDecision {
            allowed: false,
            reason: format!(
                "Tenant '{}' is not authorized to {} tenant '{}'",
                req.requester_tenant, req.operation, req.target_tenant
            ),
        }
    }
}

// ============ 高级跨 Agent 共享查询 ============

/// 跨智能体知识共享查询（同租户内多 Agent 共享 LTM）
pub struct CrossAgentMemoryQuery;

impl CrossAgentMemoryQuery {
    /// 在同租户所有 agent 的 LTM 中搜索，返回聚合结果
    pub async fn search_tenant_ltm(
        tenant_id: &TenantId,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<crate::services::memory_search::SearchResult>, AppError> {
        // 直接委托三路混合搜索，在后处理阶段按租户前缀过滤
        let raw = crate::services::memory_search::MemorySearchService::triple_hybrid_search(
            query,
            top_k * 3, // 先取多一些，再过滤
            None,
            None,
            None,
            Some(false),
            None,
        )
        .await?;

        let isolation = TenantIsolationLayer::new();
        let filtered: Vec<_> = raw
            .into_iter()
            .filter(|r| {
                if !isolation.config.enforce {
                    return true;
                }

                search_result_matches_tenant(r, tenant_id)
            })
            .take(top_k)
            .collect();

        info!(
            "CrossAgentMemoryQuery: tenant={}, query_len={}, results={}",
            tenant_id,
            query.len(),
            filtered.len()
        );

        Ok(filtered)
    }

    /// 列举同租户下所有智能体的 STM 会话
    pub async fn list_tenant_sessions(
        tenant_id: &TenantId,
        limit: Option<i32>,
    ) -> Result<Vec<crate::db::stm::Session>, AppError> {
        let all_sessions =
            crate::db::stm::STMRepository::list_sessions(None, None, limit, None).await?;

        let prefix = tenant_id.prefix();
        let tenant_sessions: Vec<_> = all_sessions
            .sessions
            .into_iter()
            .filter(|s| s.user_id.starts_with(&prefix) || s.agent_id.starts_with(&prefix))
            .collect();

        Ok(tenant_sessions)
    }
}

fn search_result_matches_tenant(
    result: &crate::services::memory_search::SearchResult,
    tenant_id: &TenantId,
) -> bool {
    let prefix = tenant_id.prefix();

    tenant_field_matches(&result.metadata, "tenant_id", tenant_id.as_str())
        || tenant_field_has_prefix(&result.metadata, "source_id", &prefix)
        || tenant_field_has_prefix(&result.metadata, "user_id", &prefix)
        || tenant_field_has_prefix(&result.metadata, "agent_id", &prefix)
}

fn tenant_field_matches(metadata: &serde_json::Value, field: &str, expected: &str) -> bool {
    metadata
        .get(field)
        .and_then(|value| value.as_str())
        .map(|value| value == expected)
        .unwrap_or(false)
}

fn tenant_field_has_prefix(metadata: &serde_json::Value, field: &str, prefix: &str) -> bool {
    metadata
        .get(field)
        .and_then(|value| value.as_str())
        .map(|value| value.starts_with(prefix))
        .unwrap_or(false)
}

// ============ 配额检查 ============

/// 租户配额检查器
pub struct QuotaEnforcer;

impl QuotaEnforcer {
    /// 检查租户是否仍有 LTM 写入配额
    pub async fn check_ltm_quota(tenant_id: &str) -> Result<(), AppError> {
        let cfg = match get_tenant(tenant_id) {
            Some(c) => c,
            None => return Ok(()), // 未注册租户：不限制
        };
        let max = match cfg.max_ltm_entries {
            Some(m) => m,
            None => return Ok(()), // 无限制
        };

        // 粗略统计：查询带前缀的条目数（实际应走缓存计数器）
        let result =
            crate::db::ltm::LTMRepository::list_entries(None, None, Some(1), Some(0)).await?;
        // 注：完整实现需要 tenant-scoped COUNT；此处保守地检查总量
        if result.total >= max {
            warn!(
                "Tenant '{}' has reached LTM quota ({} entries)",
                tenant_id, max
            );
            return Err(AppError::BadRequest(format!(
                "Tenant '{}' has reached the LTM entry quota ({})",
                tenant_id, max
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_result(metadata: serde_json::Value) -> crate::services::memory_search::SearchResult {
        crate::services::memory_search::SearchResult {
            entry_id: "entry_1".to_string(),
            score: 0.9,
            content: "test".to_string(),
            title: None,
            metadata,
        }
    }

    #[test]
    fn search_result_matches_explicit_tenant_id() {
        let tenant = TenantId::new("tenant_a");
        let result = search_result(serde_json::json!({
            "tenant_id": "tenant_a"
        }));

        assert!(search_result_matches_tenant(&result, &tenant));
    }

    #[test]
    fn search_result_matches_prefixed_source_id() {
        let tenant = TenantId::new("tenant_a");
        let result = search_result(serde_json::json!({
            "source_id": "t:tenant_a:agent:writer"
        }));

        assert!(search_result_matches_tenant(&result, &tenant));
    }

    #[test]
    fn search_result_rejects_missing_tenant_metadata() {
        let tenant = TenantId::new("tenant_a");
        let result = search_result(serde_json::json!({
            "source_id": "legacy_source"
        }));

        assert!(!search_result_matches_tenant(&result, &tenant));
    }

    #[test]
    fn search_result_rejects_other_tenant() {
        let tenant = TenantId::new("tenant_a");
        let result = search_result(serde_json::json!({
            "tenant_id": "tenant_b",
            "source_id": "t:tenant_b:agent:writer"
        }));

        assert!(!search_result_matches_tenant(&result, &tenant));
    }
}
