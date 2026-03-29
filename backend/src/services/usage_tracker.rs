//! Usage Tracking Service
//!
//! This module provides usage tracking for billing purposes.

use chrono::{Datelike, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetrics {
    pub tenant_id: String,
    pub period: BillingPeriod,
    pub api_calls: u64,
    pub storage_mb: u64,
    pub cognitive_units: u64,
    pub memory_operations: u64,
    pub vector_queries: u64,
}

/// Billing period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingPeriod {
    pub start: i64,
    pub end: i64,
}

/// Usage record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub record_id: String,
    pub tenant_id: String,
    pub metric_type: MetricType,
    pub quantity: f64,
    pub timestamp: i64,
    pub metadata: HashMap<String, String>,
}

/// Metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    ApiCall,
    StorageMb,
    CognitiveUnit,
    MemoryOperation,
    VectorQuery,
}

impl MetricType {
    pub fn unit_price(&self) -> f64 {
        match self {
            MetricType::ApiCall => 0.001,          // $0.001 per call
            MetricType::StorageMb => 0.01,         // $0.01 per MB/month
            MetricType::CognitiveUnit => 0.1,      // $0.1 per cognitive unit
            MetricType::MemoryOperation => 0.0001, // $0.0001 per operation
            MetricType::VectorQuery => 0.002,      // $0.002 per vector query
        }
    }
}

/// Subscription tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionTier {
    Free,
    Starter,
    Pro,
    Enterprise,
}

impl SubscriptionTier {
    pub fn included_api_calls(&self) -> u64 {
        match self {
            SubscriptionTier::Free => 1_000,
            SubscriptionTier::Starter => 100_000,
            SubscriptionTier::Pro => 1_000_000,
            SubscriptionTier::Enterprise => u64::MAX,
        }
    }

    pub fn included_storage_mb(&self) -> u64 {
        match self {
            SubscriptionTier::Free => 100,
            SubscriptionTier::Starter => 10_000,
            SubscriptionTier::Pro => 100_000,
            SubscriptionTier::Enterprise => u64::MAX,
        }
    }

    pub fn included_cognitive_units(&self) -> u64 {
        match self {
            SubscriptionTier::Free => 100,
            SubscriptionTier::Starter => 10_000,
            SubscriptionTier::Pro => 100_000,
            SubscriptionTier::Enterprise => u64::MAX,
        }
    }
}

/// Usage tracker service
pub struct UsageTracker {
    records: Arc<RwLock<Vec<UsageRecord>>>,
    tenants: Arc<RwLock<HashMap<String, TenantUsage>>>,
}

#[derive(Debug, Clone)]
struct TenantUsage {
    tier: SubscriptionTier,
    api_calls: u64,
    storage_mb: u64,
    cognitive_units: u64,
    memory_operations: u64,
    vector_queries: u64,
    reset_at: i64,
}

impl UsageTracker {
    /// Create a new usage tracker
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            tenants: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize tenant usage tracking
    pub async fn init_tenant(&self, tenant_id: &str, tier: SubscriptionTier) {
        let mut tenants = self.tenants.write().await;
        let reset_at = Self::next_month_start();

        tenants.insert(
            tenant_id.to_string(),
            TenantUsage {
                tier,
                api_calls: 0,
                storage_mb: 0,
                cognitive_units: 0,
                memory_operations: 0,
                vector_queries: 0,
                reset_at,
            },
        );
        info!(
            "Initialized usage tracking for tenant {} with tier {:?}",
            tenant_id, tier
        );
    }

    /// Record a usage event
    pub async fn record_usage(
        &self,
        tenant_id: &str,
        metric_type: MetricType,
        quantity: f64,
        metadata: Option<HashMap<String, String>>,
    ) -> Result<UsageRecord, crate::AppError> {
        let record = UsageRecord {
            record_id: ulid::Ulid::new().to_string(),
            tenant_id: tenant_id.to_string(),
            metric_type,
            quantity,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: metadata.unwrap_or_default(),
        };

        // Store record
        self.records.write().await.push(record.clone());

        // Update tenant usage
        let mut tenants = self.tenants.write().await;
        if let Some(usage) = tenants.get_mut(tenant_id) {
            // Check if we need to reset for new month
            let now = chrono::Utc::now().timestamp();
            if now > usage.reset_at {
                *usage = TenantUsage {
                    tier: usage.tier,
                    api_calls: 0,
                    storage_mb: 0,
                    cognitive_units: 0,
                    memory_operations: 0,
                    vector_queries: 0,
                    reset_at: Self::next_month_start(),
                };
            }

            // Update metrics
            match record.metric_type {
                MetricType::ApiCall => usage.api_calls += quantity as u64,
                MetricType::StorageMb => usage.storage_mb += quantity as u64,
                MetricType::CognitiveUnit => usage.cognitive_units += quantity as u64,
                MetricType::MemoryOperation => usage.memory_operations += quantity as u64,
                MetricType::VectorQuery => usage.vector_queries += quantity as u64,
            }
        }

        info!("Recorded {} units for tenant {}", quantity, tenant_id);
        Ok(record)
    }

    /// Get usage for a tenant in a period
    pub async fn get_usage(&self, tenant_id: &str, start: i64, end: i64) -> UsageMetrics {
        let records = self.records.read().await;

        let filtered: Vec<&UsageRecord> = records
            .iter()
            .filter(|r| r.tenant_id == tenant_id && r.timestamp >= start && r.timestamp <= end)
            .collect();

        let mut api_calls = 0u64;
        let mut storage_mb = 0u64;
        let mut cognitive_units = 0u64;
        let mut memory_operations = 0u64;
        let mut vector_queries = 0u64;

        for r in filtered {
            match r.metric_type {
                MetricType::ApiCall => api_calls += r.quantity as u64,
                MetricType::StorageMb => storage_mb += r.quantity as u64,
                MetricType::CognitiveUnit => cognitive_units += r.quantity as u64,
                MetricType::MemoryOperation => memory_operations += r.quantity as u64,
                MetricType::VectorQuery => vector_queries += r.quantity as u64,
            }
        }

        UsageMetrics {
            tenant_id: tenant_id.to_string(),
            period: BillingPeriod { start, end },
            api_calls,
            storage_mb,
            cognitive_units,
            memory_operations,
            vector_queries,
        }
    }

    /// Get current usage for tenant
    pub async fn get_current_usage(&self, tenant_id: &str) -> Option<UsageMetrics> {
        let tenants = self.tenants.read().await;
        let usage = tenants.get(tenant_id)?;

        let now = chrono::Utc::now().timestamp();
        let start = usage.reset_at;

        Some(UsageMetrics {
            tenant_id: tenant_id.to_string(),
            period: BillingPeriod { start, end: now },
            api_calls: usage.api_calls,
            storage_mb: usage.storage_mb,
            cognitive_units: usage.cognitive_units,
            memory_operations: usage.memory_operations,
            vector_queries: usage.vector_queries,
        })
    }

    /// Calculate estimated cost
    pub async fn calculate_cost(&self, tenant_id: &str) -> f64 {
        if let Some(usage) = self.get_current_usage(tenant_id).await {
            let mut cost = 0.0;
            cost += usage.api_calls as f64 * MetricType::ApiCall.unit_price();
            cost += usage.storage_mb as f64 * MetricType::StorageMb.unit_price();
            cost += usage.cognitive_units as f64 * MetricType::CognitiveUnit.unit_price();
            cost += usage.memory_operations as f64 * MetricType::MemoryOperation.unit_price();
            cost += usage.vector_queries as f64 * MetricType::VectorQuery.unit_price();
            cost
        } else {
            0.0
        }
    }

    /// Check if tenant is within quota
    pub async fn check_quota(&self, tenant_id: &str) -> QuotaStatus {
        let tenants = self.tenants.read().await;
        let usage = match tenants.get(tenant_id) {
            Some(u) => u,
            None => {
                return QuotaStatus {
                    within_quota: true,
                    overage: HashMap::new(),
                };
            }
        };

        let mut overage = HashMap::new();

        if usage.api_calls > usage.tier.included_api_calls() {
            overage.insert(
                "api_calls".to_string(),
                (usage.api_calls - usage.tier.included_api_calls()) as i64,
            );
        }

        if usage.storage_mb > usage.tier.included_storage_mb() {
            overage.insert(
                "storage_mb".to_string(),
                (usage.storage_mb - usage.tier.included_storage_mb()) as i64,
            );
        }

        if usage.cognitive_units > usage.tier.included_cognitive_units() {
            overage.insert(
                "cognitive_units".to_string(),
                (usage.cognitive_units - usage.tier.included_cognitive_units()) as i64,
            );
        }

        QuotaStatus {
            within_quota: overage.is_empty(),
            overage,
        }
    }

    fn next_month_start() -> i64 {
        let now = Utc::now();
        let next_month = if now.month() == 12 {
            Utc.with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0).unwrap()
        } else {
            Utc.with_ymd_and_hms(now.year(), now.month() + 1, 1, 0, 0, 0)
                .unwrap()
        };
        next_month.timestamp()
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Quota status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub within_quota: bool,
    pub overage: HashMap<String, i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_usage() {
        let tracker = UsageTracker::new();
        tracker
            .init_tenant("tenant_1", SubscriptionTier::Free)
            .await;

        tracker
            .record_usage("tenant_1", MetricType::ApiCall, 10.0, None)
            .await
            .unwrap();

        let usage = tracker.get_current_usage("tenant_1").await.unwrap();
        assert_eq!(usage.api_calls, 10);
    }

    #[tokio::test]
    async fn test_quota_check() {
        let tracker = UsageTracker::new();
        tracker
            .init_tenant("tenant_1", SubscriptionTier::Free)
            .await;

        // Free tier has 1000 API calls included
        for _ in 0..1001 {
            tracker
                .record_usage("tenant_1", MetricType::ApiCall, 1.0, None)
                .await
                .unwrap();
        }

        let status = tracker.check_quota("tenant_1").await;
        assert!(!status.within_quota);
        assert!(status.overage.contains_key("api_calls"));
    }

    #[tokio::test]
    async fn test_cost_calculation() {
        let tracker = UsageTracker::new();
        tracker
            .init_tenant("tenant_1", SubscriptionTier::Free)
            .await;

        tracker
            .record_usage("tenant_1", MetricType::ApiCall, 100.0, None)
            .await
            .unwrap();

        let cost = tracker.calculate_cost("tenant_1").await;
        // 100 calls * $0.001 = $0.10
        assert!((cost - 0.10).abs() < 0.001);
    }
}
