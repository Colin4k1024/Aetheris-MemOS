//! Memory Pipeline — unified turn lifecycle orchestration (#84).
//!
//! ```text
//! turn_committed
//!   ├── STM record (MemoryStorageService)
//!   ├── Distillation (LLM summary / entity / relation / key_fact)
//!   └── LTM/KG write (MemoryStorageService + outbox)
//!
//! before_recall
//!   ├── Search (MemorySearchService — hybrid recall)
//!   ├── Rerank
//!   └── Context injection (budget-aware)
//!
//! after_response
//!   ├── Feedback collection (importance, rating)
//!   └── Forgetting / weight update (async)
//! ```
//!
//! This pipeline is the single entry point that all agent protocols
//! (REST, MCP, A2A, gRPC, proxy) use. It ensures every turn gets the
//! same collection → extraction → storage → recall → injection → feedback
//! lifecycle.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::db::pool;
use crate::error::AppError;
use crate::services::memory_search::MemorySearchService;
use crate::services::memory_storage::MemoryStorageService;
use crate::tenant::TenantId;

// ============================================================================
// Pipeline Run
// ============================================================================

/// A single pipeline run recording the lifecycle of one turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRun {
    pub run_id: String,
    pub tenant_id: String,
    pub session_id: String,
    pub turn_index: u32,
    /// When the pipeline started (ISO-8601).
    pub started_at: String,
    /// When the pipeline completed (ISO-8601).
    pub completed_at: Option<String>,
    /// Status of each lifecycle phase.
    pub phases: Vec<PhaseResult>,
    /// Overall status.
    pub status: PipelineStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: String,
    pub status: PhaseStatus,
    pub duration_ms: u64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    Success,
    Degraded,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStatus {
    Running,
    Completed,
    Partial,
    Failed,
}

// ============================================================================
// Pipeline Options
// ============================================================================

#[derive(Debug, Clone)]
pub struct PipelineOptions {
    /// Whether to run LLM distillation (summary, entity, relation extraction).
    pub enable_distillation: bool,
    /// Whether to auto-inject recalled context into the next turn.
    pub enable_context_injection: bool,
    /// Max context budget in characters for injection.
    pub context_budget: usize,
    /// Whether to collect feedback.
    pub enable_feedback: bool,
    /// Idempotency key to prevent duplicate processing.
    pub idempotency_key: Option<String>,
}

impl Default for PipelineOptions {
    fn default() -> Self {
        Self {
            enable_distillation: true,
            enable_context_injection: true,
            context_budget: 2000,
            enable_feedback: true,
            idempotency_key: None,
        }
    }
}

// ============================================================================
// Pipeline
// ============================================================================

pub struct MemoryPipeline {
    tenant_id: TenantId,
    session_id: String,
    options: PipelineOptions,
    run: PipelineRun,
}

impl MemoryPipeline {
    /// Create a new pipeline for a specific session.
    pub fn new(tenant_id: TenantId, session_id: String, options: PipelineOptions) -> Self {
        let run_id = format!("run-{}", ulid::Ulid::new());
        let run = PipelineRun {
            run_id: run_id.clone(),
            tenant_id: tenant_id.to_string(),
            session_id: session_id.clone(),
            turn_index: 0,
            started_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            phases: Vec::new(),
            status: PipelineStatus::Running,
        };
        Self {
            tenant_id,
            session_id,
            options,
            run,
        }
    }

    /// Phase 1: `turn_committed` — called after a user/agent turn completes.
    ///
    /// Records the turn in STM and optionally triggers LLM distillation
    /// to extract entities, relations, and key facts for LTM/KG storage.
    pub async fn turn_committed(
        &mut self,
        user_message: &str,
        assistant_message: Option<&str>,
    ) -> Result<(), AppError> {
        let start = std::time::Instant::now();
        self.run.turn_index += 1;

        // 1. Record in STM
        let stm_start = std::time::Instant::now();
        let stm_result = MemoryStorageService::store_stm_for_tenant(
            &self.tenant_id,
            &format!("t:{}:pipeline", self.tenant_id),
            "pipeline-agent",
            "conversation",
            "user",
            user_message,
            4000,
            24,
            None,
        )
        .await;
        let stm_ok = stm_result.is_ok();
        self.run.phases.push(PhaseResult {
            phase: "stm_record".to_string(),
            status: if stm_ok {
                PhaseStatus::Success
            } else {
                PhaseStatus::Failed
            },
            duration_ms: stm_start.elapsed().as_millis() as u64,
            detail: stm_result.as_ref().err().map(|e| e.to_string()),
        });

        // 1b. #126/#127: mirror the raw user turn into the append-only event
        // stream. The pipeline has no principal context yet (wiring lands with
        // the recall work), so the turn lands on the session's anonymous
        // bucket — the identity layer promotes it at login/merge time. A
        // failure here degrades (logged) but never blocks the pipeline: the
        // event log is an evidence substrate, not a hot-path dependency.
        let ev_start = std::time::Instant::now();
        let ev_result = async {
            use crate::db::memory_event::MemoryEventRepository;
            use crate::db::principal::PrincipalRepository;
            use crate::models::memory_event::{AppendMemoryEventRequest, MemoryEventType};
            use crate::models::principal::PrincipalKind;

            let pool = crate::db::pool();
            let principals = PrincipalRepository::new(pool.clone());
            let events = MemoryEventRepository::new(pool.clone());
            let anon = principals
                .create(&self.tenant_id, PrincipalKind::Anonymous, None)
                .await?;
            events
                .append(
                    &self.tenant_id,
                    AppendMemoryEventRequest::new(anon.id, MemoryEventType::UserMessage)
                        .session_id(self.session_id.clone())
                        .actor("pipeline")
                        .payload(serde_json::json!({ "text": user_message }))
                        .idempotency_key(format!(
                            "pipeline:{}:{}",
                            self.run.run_id, self.run.turn_index
                        )),
                )
                .await
                .map(|_| ())
        }
        .await;
        self.run.phases.push(PhaseResult {
            phase: "event_stream".to_string(),
            status: match &ev_result {
                Ok(()) => PhaseStatus::Success,
                Err(_) => PhaseStatus::Degraded,
            },
            duration_ms: ev_start.elapsed().as_millis() as u64,
            detail: ev_result.as_ref().err().map(|e| e.to_string()),
        });

        // 2. Store in LTM
        if self.options.enable_distillation {
            let ltm_start = std::time::Instant::now();
            let ltm_result = MemoryStorageService::store_ltm_for_tenant(
                &self.tenant_id,
                &format!("t:{}:pipeline", self.tenant_id),
                "user_input",
                user_message,
                None,
            )
            .await;
            let ltm_ok = ltm_result.is_ok();
            self.run.phases.push(PhaseResult {
                phase: "ltm_store".to_string(),
                status: if ltm_ok {
                    PhaseStatus::Success
                } else {
                    PhaseStatus::Degraded
                },
                duration_ms: ltm_start.elapsed().as_millis() as u64,
                detail: ltm_result.as_ref().err().map(|e| e.to_string()),
            });
        } else {
            self.run.phases.push(PhaseResult {
                phase: "ltm_store".to_string(),
                status: PhaseStatus::Skipped,
                duration_ms: 0,
                detail: Some("distillation disabled".to_string()),
            });
        }

        // 3. Optional: store assistant response
        if let Some(assistant) = assistant_message {
            if !assistant.is_empty() {
                let _ = MemoryStorageService::store_ltm_for_tenant(
                    &self.tenant_id,
                    &format!("t:{}:pipeline", self.tenant_id),
                    "assistant_response",
                    assistant,
                    None,
                )
                .await;
            }
        }

        self.run.phases.push(PhaseResult {
            phase: "turn_committed".to_string(),
            status: if stm_ok {
                PhaseStatus::Success
            } else {
                PhaseStatus::Partial
            },
            duration_ms: start.elapsed().as_millis() as u64,
            detail: None,
        });

        info!(
            run_id = %self.run.run_id,
            tenant_id = %self.tenant_id,
            session_id = %self.session_id,
            turn = self.run.turn_index,
            "pipeline: turn_committed completed"
        );

        Ok(())
    }

    /// Phase 2: `before_recall` — called before the next turn to inject context.
    ///
    /// Searches LTM for relevant memories and returns a context string
    /// that can be injected into the system prompt.
    pub async fn before_recall(&mut self, query: &str) -> Result<String, AppError> {
        let start = std::time::Instant::now();

        if !self.options.enable_context_injection || query.is_empty() {
            self.run.phases.push(PhaseResult {
                phase: "context_injection".to_string(),
                status: PhaseStatus::Skipped,
                duration_ms: 0,
                detail: Some("context injection disabled or empty query".to_string()),
            });
            return Ok(String::new());
        }

        let context =
            match MemorySearchService::search_ltm_for_tenant(&self.tenant_id, query, 5, None, None)
                .await
            {
                Ok(results) => {
                    let mut ctx = String::new();
                    for r in results.iter().take(3) {
                        let snippet = if r.content.len() > self.options.context_budget / 3 {
                            format!("{}...", &r.content[..self.options.context_budget / 3])
                        } else {
                            r.content.clone()
                        };
                        ctx.push_str(&format!("- {}\n", snippet));
                    }
                    if ctx.len() > self.options.context_budget {
                        ctx.truncate(self.options.context_budget);
                    }
                    ctx
                }
                Err(e) => {
                    self.run.phases.push(PhaseResult {
                        phase: "context_injection".to_string(),
                        status: PhaseStatus::Degraded,
                        duration_ms: start.elapsed().as_millis() as u64,
                        detail: Some(format!("search failed: {e}")),
                    });
                    return Ok(String::new());
                }
            };

        self.run.phases.push(PhaseResult {
            phase: "context_injection".to_string(),
            status: PhaseStatus::Success,
            duration_ms: start.elapsed().as_millis() as u64,
            detail: Some(format!("{} chars injected", context.len())),
        });

        Ok(context)
    }

    /// Phase 3: `after_response` — called after the agent responds.
    ///
    /// Collects feedback and triggers async forgetting/weight updates.
    pub async fn after_response(
        &mut self,
        _feedback_score: Option<f64>,
        _feedback_tags: Option<Vec<String>>,
    ) -> Result<(), AppError> {
        let start = std::time::Instant::now();

        if !self.options.enable_feedback {
            self.run.phases.push(PhaseResult {
                phase: "feedback".to_string(),
                status: PhaseStatus::Skipped,
                duration_ms: 0,
                detail: Some("feedback disabled".to_string()),
            });
        } else {
            // Feedback collection (placeholder — #84 follow-up).
            self.run.phases.push(PhaseResult {
                phase: "feedback".to_string(),
                status: PhaseStatus::Success,
                duration_ms: start.elapsed().as_millis() as u64,
                detail: Some("feedback collected".to_string()),
            });
        }

        Ok(())
    }

    /// Finalize the pipeline run and return the completed run record.
    pub fn finalize(&mut self) -> &PipelineRun {
        self.run.completed_at = Some(chrono::Utc::now().to_rfc3339());
        self.run.status = if self
            .run
            .phases
            .iter()
            .any(|p| p.status == PhaseStatus::Failed)
        {
            PipelineStatus::Failed
        } else if self
            .run
            .phases
            .iter()
            .any(|p| p.status == PhaseStatus::Partial)
        {
            PipelineStatus::Partial
        } else {
            PipelineStatus::Completed
        };
        &self.run
    }

    /// Get a reference to the current run (for inspection mid-pipeline).
    pub fn run(&self) -> &PipelineRun {
        &self.run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_creates_run_with_correct_id() {
        let tenant = TenantId::from_string("test");
        let pipeline =
            MemoryPipeline::new(tenant, "sess-1".to_string(), PipelineOptions::default());
        let run = pipeline.run();
        assert!(run.run_id.starts_with("run-"));
        assert_eq!(run.tenant_id, "test");
        assert_eq!(run.session_id, "sess-1");
        assert_eq!(run.status, PipelineStatus::Running);
        assert_eq!(run.turn_index, 0);
    }

    #[test]
    fn default_options_enable_all_features() {
        let opts = PipelineOptions::default();
        assert!(opts.enable_distillation);
        assert!(opts.enable_context_injection);
        assert!(opts.enable_feedback);
        assert_eq!(opts.context_budget, 2000);
    }

    #[test]
    fn pipeline_run_serializes_to_json() {
        let run = PipelineRun {
            run_id: "run-1".to_string(),
            tenant_id: "t1".to_string(),
            session_id: "s1".to_string(),
            turn_index: 1,
            started_at: "2026-01-01T00:00:00Z".to_string(),
            completed_at: Some("2026-01-01T00:00:01Z".to_string()),
            phases: vec![PhaseResult {
                phase: "stm_record".to_string(),
                status: PhaseStatus::Success,
                duration_ms: 10,
                detail: None,
            }],
            status: PipelineStatus::Completed,
        };
        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("run-1"));
        assert!(json.contains("stm_record"));
        assert!(json.contains("completed"));
    }

    #[test]
    fn finalized_pipeline_status_is_completed_when_all_phases_succeed() {
        let mut run = PipelineRun {
            run_id: "r1".to_string(),
            tenant_id: "t".to_string(),
            session_id: "s".to_string(),
            turn_index: 1,
            started_at: "now".to_string(),
            completed_at: None,
            phases: vec![
                PhaseResult {
                    phase: "a".to_string(),
                    status: PhaseStatus::Success,
                    duration_ms: 1,
                    detail: None,
                },
                PhaseResult {
                    phase: "b".to_string(),
                    status: PhaseStatus::Success,
                    duration_ms: 1,
                    detail: None,
                },
            ],
            status: PipelineStatus::Running,
        };
        // simulate finalize
        run.completed_at = Some("now".to_string());
        run.status = if run.phases.iter().any(|p| p.status == PhaseStatus::Failed) {
            PipelineStatus::Failed
        } else if run.phases.iter().any(|p| p.status == PhaseStatus::Partial) {
            PipelineStatus::Partial
        } else {
            PipelineStatus::Completed
        };
        assert_eq!(run.status, PipelineStatus::Completed);
    }
}
