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
                        Self::process_l1_to_l2(tenant_id, &job.user_id, &job.agent_id).await
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

        Ok(created_count)
    }

    async fn process_l1_to_l2(
        _tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
    ) -> Result<i32, AppError> {
        // Placeholder -- Phase 2 will implement SceneConsolidator
        info!(
            "L1->L2 consolidation placeholder for {}/{}",
            user_id, agent_id
        );
        Ok(0)
    }

    async fn process_l2_to_l3(
        _tenant_id: &TenantId,
        user_id: &str,
        agent_id: &str,
    ) -> Result<i32, AppError> {
        // Placeholder -- Phase 2 will implement PersonaBuilder
        info!(
            "L2->L3 persona build placeholder for {}/{}",
            user_id, agent_id
        );
        Ok(0)
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
