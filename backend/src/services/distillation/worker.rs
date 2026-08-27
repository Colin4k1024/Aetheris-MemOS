use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::config;
use crate::db::distillation::DistillationRepository;
use crate::db::stm::STMRepository;
use crate::models::distillation::DistillationJobType;
use crate::services::atom_extractor::AtomExtractor;
use crate::tenant::TenantId;
use crate::AppError;

pub struct DistillationService {
    running: bool,
    running_flag: Option<Arc<AtomicBool>>,
}

impl DistillationService {
    pub fn new() -> Self {
        Self {
            running: false,
            running_flag: None,
        }
    }

    /// Enqueue a new distillation job for async processing
    pub async fn enqueue_job(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        session_id: &str,
        job_type: DistillationJobType,
    ) -> Result<String, AppError> {
        let config = config::get();
        if !config.distillation.enabled {
            info!("Distillation disabled, skipping job enqueue");
            return Ok(String::new());
        }

        let job_id = DistillationRepository::create_job(
            tenant_id,
            user_id,
            agent_id,
            session_id,
            job_type.as_str(),
        )
        .await?;

        info!(
            "Enqueued distillation job: id={}, type={}, session={}",
            job_id, job_type, session_id
        );
        Ok(job_id)
    }

    /// Start the background worker
    pub async fn start(&mut self) -> Result<(), AppError> {
        if self.running {
            warn!("Distillation service is already running");
            return Ok(());
        }

        let config = config::get();
        if !config.distillation.enabled {
            info!("Distillation service disabled by config");
            return Ok(());
        }

        self.running = true;
        let poll_interval = config.distillation.worker_poll_interval_seconds;
        let running_flag = Arc::new(AtomicBool::new(true));
        let running_flag_clone = running_flag.clone();
        self.running_flag = Some(running_flag);

        info!(
            "Starting distillation worker: poll_interval={}s",
            poll_interval
        );

        tokio::spawn(async move {
            while running_flag_clone.load(Ordering::Relaxed) {
                // Process jobs for all tenants
                if let Err(e) = Self::process_pending_jobs().await {
                    error!("Distillation worker error: {}", e);
                }
                sleep(Duration::from_secs(poll_interval)).await;
            }
            info!("Distillation worker loop exited");
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        self.running = false;
        if let Some(flag) = &self.running_flag {
            flag.store(false, Ordering::Relaxed);
            self.running_flag = None;
        }
        info!("Distillation service stopped");
    }

    async fn process_pending_jobs() -> Result<(), AppError> {
        let tenants = crate::services::multi_tenant::list_scheduled_tenants();

        for tenant_id in &tenants {
            // Process one job per tenant per cycle
            if let Some(job) = DistillationRepository::claim_next_job(tenant_id).await? {
                info!(
                    "Processing distillation job: id={}, type={}, session={}",
                    job.id, job.job_type, job.session_id
                );

                let result = match job.job_type.as_str() {
                    "l0_to_l1" => {
                        Self::process_l0_to_l1(
                            tenant_id,
                            &job.user_id,
                            &job.agent_id,
                            &job.session_id,
                        )
                        .await
                    }
                    "l1_to_l2" => {
                        // The L1→L2 job carries the scene_name in `session_id`
                        // (see process_l0_to_l1's enqueue).
                        Self::process_l1_to_l2(
                            tenant_id,
                            &job.user_id,
                            &job.agent_id,
                            &job.session_id,
                        )
                        .await
                    }
                    "l2_to_l3" => {
                        Self::process_l2_to_l3(tenant_id, &job.user_id, &job.agent_id).await
                    }
                    _ => {
                        warn!("Unknown job type: {}", job.job_type);
                        Ok(0)
                    }
                };

                match result {
                    Ok(atoms_created) => {
                        DistillationRepository::complete_job(&job.id, atoms_created).await?;
                        info!("Job {} completed: {} atoms created", job.id, atoms_created);
                    }
                    Err(e) => {
                        let err_msg = format!("{}", e);
                        error!("Job {} failed: {}", job.id, err_msg);
                        DistillationRepository::fail_job(&job.id, &err_msg).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn process_l0_to_l1(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        session_id: &str,
    ) -> Result<i32, AppError> {
        let config = config::get();
        let pool = crate::db::pool();

        // Load session messages
        let messages =
            STMRepository::get_session_messages(pool, tenant_id, session_id, None).await?;

        if (messages.len() as u32) < config.distillation.min_message_count {
            info!(
                "Session {} has only {} messages, below threshold {}",
                session_id,
                messages.len(),
                config.distillation.min_message_count
            );
            return Ok(0);
        }

        // Extract atoms via LLM
        let extraction = AtomExtractor::extract_from_messages(&messages)
            .await
            .map_err(|e| AppError::Internal(format!("Atom extraction failed: {}", e)))?;

        let mut created_count = 0i32;
        for atom in &extraction.atoms {
            let metadata = serde_json::json!({});
            DistillationRepository::create_atom(
                tenant_id,
                user_id,
                agent_id,
                &atom.atom_type,
                &atom.scene_name,
                &atom.content,
                atom.priority,
                session_id,
                &atom.source_message_ids,
                &metadata,
            )
            .await?;
            created_count += 1;
        }

        // Check if any scenes need consolidation
        let scene_threshold = config.distillation.scene_consolidation_atom_threshold;
        for segment in &extraction.scene_segments {
            let count = DistillationRepository::count_atoms_for_scene(
                tenant_id,
                user_id,
                agent_id,
                &segment.scene_name,
            )
            .await?;
            if count >= scene_threshold as i64 {
                // Enqueue L1->L2 job for this scene
                Self::enqueue_job(
                    tenant_id,
                    user_id,
                    agent_id,
                    &segment.scene_name, // Re-use session_id field for scene_name
                    DistillationJobType::L1ToL2,
                )
                .await?;
            }
        }

        // #127: the same session also feeds the belief write gate. The
        // deterministic producer pattern-matches allowlist predicates from the
        // raw user turns; each claim is then gated (allowlist → source policy →
        // probe → evidence → precedence) and becomes a belief candidate. This
        // replaces "the whole turn becomes a long-term fact" with auditable,
        // supersede-managed SPO edges. Failures degrade to a log line: belief
        // extraction must never take the distillation job down with it.
        let _ = Self::produce_belief_candidates(tenant_id, user_id, session_id, &messages).await;

        Ok(created_count)
    }

    /// #127 wiring: derive governed claims from a session's user turns and
    /// submit them through the belief gate. Requires the subject principal to
    /// exist; when it does not (pre-#128 deployments), the step is skipped.
    async fn produce_belief_candidates(
        tenant_id: &TenantId,
        user_id: &str,
        session_id: &str,
        messages: &[crate::db::stm::SessionMessage],
    ) -> Result<usize, AppError> {
        use crate::db::belief::BeliefRepository;
        use crate::db::memory_event::MemoryEventRepository;
        use crate::db::principal::PrincipalRepository;
        use crate::models::belief::BeliefSource;
        use crate::models::memory_event::AppendMemoryEventRequest;
        use crate::models::memory_event::MemoryEventType;
        use crate::models::principal::{PrincipalAliasType, PrincipalKind};
        use crate::services::belief::BeliefGateService;

        let pool = crate::db::pool();
        let principals = PrincipalRepository::new(pool.clone());
        let Some(principal) = principals
            .find_by_alias(tenant_id, PrincipalAliasType::JwtSub, user_id)
            .await?
        else {
            tracing::debug!(
                tenant = %tenant_id.as_str(),
                user_id,
                "belief extraction skipped: no principal mapped for user yet"
            );
            return Ok(0);
        };

        let events = MemoryEventRepository::new(pool.clone());
        let gate = BeliefGateService::new(pool.clone());
        let mut submitted = 0usize;

        for msg in messages {
            if msg.role != "user" || msg.content.trim().is_empty() {
                continue;
            }
            // Evidence first: the immutable event anchors the claim's provenance.
            let event = events
                .append(
                    tenant_id,
                    AppendMemoryEventRequest::new(
                        principal.id.clone(),
                        MemoryEventType::UserMessage,
                    )
                    .session_id(session_id)
                    .actor(user_id)
                    .payload(serde_json::json!({ "text": msg.content }))
                    .idempotency_key(format!("distill:{session_id}:{}", msg.message_id)),
                )
                .await?;
            let event_id = event.id().to_string();

            for mut claim in BeliefGateService::claims_from_message(
                &principal.id,
                Some(session_id),
                &msg.content,
                BeliefSource::UserStated,
            ) {
                claim.evidence_event_ids = vec![event_id.clone()];
                match gate.submit(tenant_id, claim).await {
                    Ok(_) => submitted += 1,
                    Err(e) => {
                        tracing::warn!(
                            tenant = %tenant_id.as_str(),
                            error = %e,
                            "belief claim rejected by gate infrastructure (kept as candidate)"
                        );
                    }
                }
            }
        }

        if submitted > 0 {
            info!(
                tenant = %tenant_id.as_str(),
                session_id,
                submitted,
                "belief candidates submitted through write gate"
            );
        }
        Ok(submitted)
    }

    async fn process_l1_to_l2(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
        scene_name: &str,
    ) -> Result<i32, AppError> {
        // Consolidate one scene's L1 atoms into an L2 scene document via the
        // LLM. The compute (prompt + response parse) is reused from the SQLite
        // path's l2_consolidator — it is backend-agnostic; only the atom/scene
        // types differ, and we adapt them here to the PG L1Atom/L2Scene.
        let atoms =
            DistillationRepository::get_atoms_for_scene(tenant_id, user_id, agent_id, scene_name)
                .await?;
        if atoms.is_empty() {
            info!("L1->L2: no atoms for scene {}, skipping", scene_name);
            return Ok(0);
        }

        let atoms_text = atoms
            .iter()
            .map(|a| {
                format!(
                    "- [{}] (scene: {}, type: {}, priority: {}) {}",
                    a.id, a.scene_name, a.atom_type, a.priority, a.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Existing scenes let the LLM avoid duplicating them.
        let existing = DistillationRepository::list_scenes(tenant_id, user_id, agent_id)
            .await
            .unwrap_or_default();
        let scenes_text = if existing.is_empty() {
            "(无现有场景)".to_string()
        } else {
            existing
                .iter()
                .map(|s| format!("- [{}] {} — {}", s.id, s.scene_name, s.content))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let user_prompt =
            super::prompts::format_l2_consolidation_user_prompt(&atoms_text, &scenes_text);
        let full_prompt = format!(
            "{}\n\n{}",
            super::prompts::L2_CONSOLIDATION_SYSTEM_PROMPT,
            user_prompt
        );

        let llm = crate::services::llm::get_llm_service()
            .map_err(|e| AppError::Internal(format!("LLM service unavailable: {e}")))?;
        let response = llm
            .call_llm_public(&full_prompt)
            .await
            .map_err(|e| AppError::Internal(format!("L2 consolidation LLM call failed: {e}")))?;

        let scene_updates = super::l2_consolidator::parse_consolidation_response(&response)
            .map_err(|e| AppError::Internal(format!("L2 consolidation parse failed: {e}")))?;

        let mut upserted = 0i32;
        for update in &scene_updates {
            // Rough token estimate (~4 chars/token); good enough for budgeting.
            let token_count = (update.content.chars().count() as i32) / 4;
            let _ = DistillationRepository::upsert_scene(
                tenant_id,
                user_id,
                agent_id,
                &update.name,
                &update.name,
                &update.content,
                &update.atom_ids,
                token_count,
            )
            .await;
            upserted += 1;
        }

        // After consolidating, schedule L3 persona regeneration if the user has
        // enough scenes. No dedup — upsert_persona is idempotent (version bump);
        // a duplicate L2ToL3 just regenerates. Dedup is a follow-up.
        let persona_threshold = config::get().distillation.persona_rebuild_scene_threshold;
        if (existing.len() as u32) >= persona_threshold {
            let _ = Self::enqueue_job(
                tenant_id,
                user_id,
                agent_id,
                "",
                DistillationJobType::L2ToL3,
            )
            .await;
        }

        info!(
            "L1->L2 consolidated scene {} for {}/{}: {} scenes upserted from {} atoms",
            scene_name,
            user_id,
            agent_id,
            upserted,
            atoms.len()
        );
        Ok(upserted)
    }

    async fn process_l2_to_l3(
        tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
    ) -> Result<i32, AppError> {
        // Generate (or regenerate) the user's L3 persona from their L2 scenes
        // via the LLM. The compute (prompt) is reused from the SQLite path's
        // l3_persona; the LLM returns the persona text directly (no JSON parse,
        // unlike L2). upsert_persona bumps version on conflict (idempotent).
        let scenes = DistillationRepository::list_scenes(tenant_id, user_id, agent_id)
            .await
            .unwrap_or_default();
        if scenes.is_empty() {
            info!("L2->L3: no scenes for {}/{}, skipping", user_id, agent_id);
            return Ok(0);
        }

        let existing = DistillationRepository::get_persona(tenant_id, user_id, agent_id)
            .await
            .unwrap_or_default();
        let existing_text = existing
            .as_ref()
            .map(|p| p.profile_content.as_str())
            .unwrap_or("(尚无画像)");

        let scene_contents = scenes
            .iter()
            .map(|s| format!("## {}\n{}\n", s.scene_name, s.content))
            .collect::<Vec<_>>()
            .join("\n---\n");

        let user_prompt =
            super::prompts::format_l3_persona_user_prompt(existing_text, &scene_contents);
        let full_prompt = format!(
            "{}\n\n{}",
            super::prompts::L3_PERSONA_SYSTEM_PROMPT,
            user_prompt
        );

        let llm = crate::services::llm::get_llm_service()
            .map_err(|e| AppError::Internal(format!("LLM service unavailable: {e}")))?;
        let persona_content = llm
            .call_llm_public(&full_prompt)
            .await
            .map_err(|e| AppError::Internal(format!("L3 persona LLM call failed: {e}")))?;

        let scene_ids: Vec<String> = scenes.iter().map(|s| s.id.clone()).collect();
        let token_count = (persona_content.chars().count() as i32) / 4;

        let _ = DistillationRepository::upsert_persona(
            tenant_id,
            user_id,
            agent_id,
            persona_content.trim(),
            &scene_ids,
            token_count,
        )
        .await;

        info!(
            "L2->L3 persona generated for {}/{}: {} scenes, content_len={}",
            user_id,
            agent_id,
            scene_ids.len(),
            persona_content.len()
        );
        Ok(1)
    }
}

/// Global service instance
static DISTILLATION_SERVICE: once_cell::sync::OnceCell<std::sync::Mutex<DistillationService>> =
    once_cell::sync::OnceCell::new();

/// Initialize and start the distillation service
pub async fn init_distillation_service() -> Result<(), AppError> {
    let mut service = DistillationService::new();
    service.start().await?;

    DISTILLATION_SERVICE
        .set(std::sync::Mutex::new(service))
        .map_err(|_| AppError::Internal("Distillation service already initialized".to_string()))?;

    Ok(())
}
