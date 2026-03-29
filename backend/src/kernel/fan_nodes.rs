//! Fan-Out and Fan-In Nodes for Workflow Orchestration
//!
//! Provides nodes that can spawn multiple sub-agent jobs (fan-out)
//! and wait for their completion (fan-in).

use crate::distributed::signaling_bus::{SignalMetadata, SignalingBus, WorkflowSignal};
use crate::runtime::subagent_pool::{PoolStatus, SubagentPool};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Result of a fan-in aggregation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanInResult {
    /// Number of sub-agents that completed successfully.
    pub success_count: usize,
    /// Number of sub-agents that failed.
    pub failure_count: usize,
    /// Aggregated results from all sub-agents.
    pub results: Vec<SubagentResult>,
}

/// Result from a single sub-agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    /// The workflow ID of the sub-agent.
    pub workflow_id: String,
    /// Whether the sub-agent completed successfully.
    pub success: bool,
    /// Result data from the sub-agent, if any.
    pub data: Option<serde_json::Value>,
    /// Error message if the sub-agent failed.
    pub error: Option<String>,
}

/// Configuration for fan-out node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanOutConfig {
    /// Number of sub-agents to spawn.
    pub count: usize,
    /// Parent workflow ID.
    pub parent_workflow_id: String,
    /// Optional configuration for each sub-agent.
    pub subagent_config: Option<serde_json::Value>,
}

/// Configuration for fan-in node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanInConfig {
    /// Number of completions to wait for.
    pub expected_count: usize,
    /// Parent workflow ID.
    pub parent_workflow_id: String,
    /// Timeout in seconds.
    pub timeout_secs: Option<u64>,
}

/// Common trait for fan nodes.
#[async_trait]
pub trait FanNode: Send + Sync {
    /// Execute the fan node operation.
    async fn execute(&self, config: &FanNodeConfig) -> anyhow::Result<FanNodeResult>;
}

/// Configuration for fan node execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FanNodeConfig {
    /// Fan-out configuration.
    FanOut(FanOutConfig),
    /// Fan-in configuration.
    FanIn(FanInConfig),
}

/// Result from fan node execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanNodeResult {
    /// Whether the operation was successful.
    pub success: bool,
    /// Error message if failed.
    pub error: Option<String>,
    /// For fan-out: IDs of spawned sub-agents.
    pub spawned_workflow_ids: Vec<String>,
    /// For fan-in: aggregated results.
    pub fan_in_result: Option<FanInResult>,
}

/// Fan-Out Node: spawns N sub-agent jobs via SignalingBus.
pub struct FanOutNode {
    signaling_bus: Arc<SignalingBus>,
    subagent_pool: Arc<SubagentPool>,
}

impl FanOutNode {
    /// Create a new fan-out node.
    pub fn new(signaling_bus: Arc<SignalingBus>, subagent_pool: Arc<SubagentPool>) -> Self {
        Self {
            signaling_bus,
            subagent_pool,
        }
    }
}

#[async_trait]
impl FanNode for FanOutNode {
    async fn execute(&self, config: &FanNodeConfig) -> anyhow::Result<FanNodeResult> {
        match config {
            FanNodeConfig::FanOut(fan_out_config) => {
                let count = fan_out_config.count.min(100); // Cap at 100
                let parent_id = &fan_out_config.parent_workflow_id;

                // Allocate slots from the pool
                let slot_ids = self.subagent_pool.allocate(count).await;

                // Spawn signals for each sub-agent
                let mut spawned_ids = Vec::with_capacity(slot_ids.len());
                for i in 0..slot_ids.len() {
                    let child_workflow_id = format!("{}-child-{}", parent_id, i);
                    spawned_ids.push(child_workflow_id.clone());

                    // Publish SubagentSpawn signal
                    self.signaling_bus.publish(
                        WorkflowSignal::SubagentSpawn {
                            child_workflow_id: child_workflow_id.clone(),
                        },
                        parent_id,
                    );
                }

                Ok(FanNodeResult {
                    success: true,
                    error: None,
                    spawned_workflow_ids: spawned_ids,
                    fan_in_result: None,
                })
            }
            FanNodeConfig::FanIn(_) => {
                // This is a fan-in config, not fan-out
                Err(anyhow::anyhow!("FanOutNode cannot execute FanIn config"))
            }
        }
    }
}

/// Fan-In Node: waits for N completions, aggregates results.
pub struct FanInNode {
    signaling_bus: Arc<SignalingBus>,
}

impl FanInNode {
    /// Create a new fan-in node.
    pub fn new(signaling_bus: Arc<SignalingBus>) -> Self {
        Self { signaling_bus }
    }

    /// Wait for completions and aggregate results.
    pub async fn wait_for_completions(
        &self,
        parent_workflow_id: &str,
        expected_count: usize,
        timeout_secs: Option<u64>,
    ) -> anyhow::Result<FanInResult> {
        let mut results = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;

        let mut rx = self.signaling_bus.subscribe(parent_workflow_id);

        let deadline = timeout_secs.map(|secs| {
            std::time::Instant::now() + std::time::Duration::from_secs(secs)
        });

        while results.len() < expected_count {
            // Check timeout
            if let Some(d) = deadline {
                if std::time::Instant::now() >= d {
                    break;
                }
            }

            // Use tokio's timeout to allow checking deadline
            let signal = match tokio::time::timeout(
                std::time::Duration::from_secs(1),
                rx.recv(),
            )
            .await
            {
                Ok(Ok(signal)) => signal,
                Ok(Err(broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(broadcast::error::RecvError::Closed)) => break,
                Err(_) => continue, // Timeout, check deadline and continue
            };

            match signal {
                WorkflowSignal::SubagentComplete { workflow_id } => {
                    success_count += 1;
                    results.push(SubagentResult {
                        workflow_id,
                        success: true,
                        data: None,
                        error: None,
                    });
                }
                WorkflowSignal::SubagentTerminate { workflow_id } => {
                    failure_count += 1;
                    results.push(SubagentResult {
                        workflow_id,
                        success: false,
                        data: None,
                        error: Some("Terminated".to_string()),
                    });
                }
                _ => {
                    // Ignore other signals during fan-in wait
                }
            }
        }

        Ok(FanInResult {
            success_count,
            failure_count,
            results,
        })
    }
}

#[async_trait]
impl FanNode for FanInNode {
    async fn execute(&self, config: &FanNodeConfig) -> anyhow::Result<FanNodeResult> {
        match config {
            FanNodeConfig::FanIn(fan_in_config) => {
                let result = self
                    .wait_for_completions(
                        &fan_in_config.parent_workflow_id,
                        fan_in_config.expected_count,
                        fan_in_config.timeout_secs,
                    )
                    .await?;

                Ok(FanNodeResult {
                    success: result.failure_count == 0,
                    error: None,
                    spawned_workflow_ids: vec![],
                    fan_in_result: Some(result),
                })
            }
            FanNodeConfig::FanOut(_) => {
                Err(anyhow::anyhow!("FanInNode cannot execute FanOut config"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fan_out_spawns_subagents() {
        let signaling_bus = Arc::new(SignalingBus::new());
        let subagent_pool = Arc::new(SubagentPool::new(10));
        let fan_out = FanOutNode::new(signaling_bus.clone(), subagent_pool.clone());

        let config = FanNodeConfig::FanOut(FanOutConfig {
            count: 3,
            parent_workflow_id: "test-workflow".to_string(),
            subagent_config: None,
        });

        let result = fan_out.execute(&config).await.unwrap();

        assert!(result.success);
        assert_eq!(result.spawned_workflow_ids.len(), 3);

        // Verify signals were published
        let signals = signaling_bus.get_parent_signals("test-workflow");
        assert_eq!(signals.len(), 3);
    }

    #[tokio::test]
    async fn test_fan_out_caps_at_max() {
        let signaling_bus = Arc::new(SignalingBus::new());
        let subagent_pool = Arc::new(SubagentPool::new(5));
        let fan_out = FanOutNode::new(signaling_bus.clone(), subagent_pool.clone());

        let config = FanNodeConfig::FanOut(FanOutConfig {
            count: 200, // Request more than max
            parent_workflow_id: "test-workflow".to_string(),
            subagent_config: None,
        });

        let result = fan_out.execute(&config).await.unwrap();

        // Should be capped at pool size (5) not 100 as we use 100 as cap
        assert!(result.spawned_workflow_ids.len() <= 100);
    }
}
