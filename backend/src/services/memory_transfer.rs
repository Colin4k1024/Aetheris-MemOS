#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, instrument, warn};

use crate::AppError;
use crate::db::stm::{STMRepository, Session};
use crate::services::memory_storage::MemoryStorageService;

/// 记忆自动转移服务
pub struct MemoryTransferService {
    /// 检查间隔（秒）
    check_interval: u64,
    /// 消息数量阈值
    message_count_threshold: i32,
    /// 会话时间阈值（小时）
    session_time_threshold: i32,
    /// 是否运行中
    running: bool,
    /// 运行标志，用于停止异步任务
    running_flag: Option<Arc<AtomicBool>>,
}

impl MemoryTransferService {
    /// 创建新的自动转移服务
    pub fn new(
        check_interval: u64,
        message_count_threshold: i32,
        session_time_threshold: i32,
    ) -> Self {
        Self {
            check_interval,
            message_count_threshold,
            session_time_threshold,
            running: false,
            running_flag: None,
        }
    }

    /// 启动自动转移服务
    pub async fn start(&mut self) -> Result<(), AppError> {
        if self.running {
            warn!("Memory transfer service is already running");
            return Ok(());
        }

        self.running = true;
        info!(
            "Starting memory transfer service: check_interval={}s, message_threshold={}, time_threshold={}h",
            self.check_interval, self.message_count_threshold, self.session_time_threshold
        );

        let check_interval = self.check_interval;
        let message_count_threshold = self.message_count_threshold;
        let session_time_threshold = self.session_time_threshold;
        let running_flag = Arc::new(AtomicBool::new(true));
        let running_flag_clone = running_flag.clone();

        // 保存运行标志到结构体中
        self.running_flag = Some(running_flag);

        tokio::spawn(async move {
            while running_flag_clone.load(Ordering::Relaxed) {
                sleep(Duration::from_secs(check_interval)).await;

                // 重试机制：最多重试3次
                let max_retries = 3;
                let mut last_error = None;

                for attempt in 1..=max_retries {
                    match Self::check_and_transfer(message_count_threshold, session_time_threshold).await {
                        Ok(_) => {
                            break; // 成功，跳出重试循环
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            last_error = Some(e);
                            if attempt < max_retries {
                                error!(
                                    attempt = %attempt,
                                    error = %error_msg,
                                    "Memory transfer check failed, retrying..."
                                );
                                sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
                            }
                        }
                    }
                }

                if let Some(e) = last_error {
                    error!(
                        error = %e,
                        "Memory transfer check failed after max retries"
                    );
                    // TODO: 可以添加错误上报到监控系统
                }
            }
            info!("Memory transfer service loop exited");
        });

        Ok(())
    }

    /// 停止自动转移服务
    pub fn stop(&mut self) {
        self.running = false;

        // 停止运行中的任务
        if let Some(running_flag) = &self.running_flag {
            running_flag.store(false, Ordering::Relaxed);
            self.running_flag = None;
        }

        info!("Memory transfer service stopped");
    }

    /// 检查并转移符合条件的会话
    #[instrument]
    async fn check_and_transfer(
        message_count_threshold: i32,
        session_time_threshold: i32,
    ) -> Result<(), AppError> {
        info!("Checking sessions for transfer to LTM");

        // 动态获取所有活跃的用户和智能体
        let user_ids = STMRepository::get_active_user_ids().await?;

        if user_ids.is_empty() {
            info!("No active sessions found");
            return Ok(());
        }

        let mut transferred_count = 0;
        for user_id in &user_ids {
            // 动态获取该用户的活跃 agent 列表
            let agent_ids = STMRepository::get_active_agent_ids(user_id).await?;

            for agent_id in &agent_ids {
                let sessions =
                    STMRepository::get_recent_sessions(user_id, agent_id, Some(100)).await?;
                info!(
                    "Found {} active sessions for user {} and agent {}",
                    sessions.len(),
                    user_id,
                    agent_id
                );

                for session in sessions {
                    // 检查是否应该转移
                    if Self::should_transfer(
                        &session,
                        message_count_threshold,
                        session_time_threshold,
                    ) {
                        info!(
                            "Transferring session to LTM: session_id={}",
                            session.session_id
                        );

                        match MemoryStorageService::auto_transfer_stm_to_ltm(
                            &session.session_id,
                            message_count_threshold,
                        )
                        .await
                        {
                            Ok(entry_ids) => {
                                transferred_count += entry_ids.len();
                                info!(
                                    "Successfully transferred session {} to LTM: {} entries created",
                                    session.session_id,
                                    entry_ids.len()
                                );
                            }
                            Err(e) => {
                                error!("Failed to transfer session {}: {}", session.session_id, e);
                            }
                        }
                    }
                }
            }
        }

        if transferred_count > 0 {
            info!(
                "Transfer cycle completed: {} entries transferred",
                transferred_count
            );
        }

        Ok(())
    }

    /// 判断会话是否应该转移到 LTM
    fn should_transfer(
        session: &Session,
        message_count_threshold: i32,
        session_time_threshold: i32,
    ) -> bool {
        // 检查消息数量
        if session.context_length >= message_count_threshold {
            info!(
                "Session {} meets message count threshold: {} >= {}",
                session.session_id, session.context_length, message_count_threshold
            );
            return true;
        }

        // 检查会话时间：计算会话持续时间（小时）
        let now = chrono::Utc::now();

        // 尝试解析会话创建时间
        if let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&session.created_at) {
            let created_at_utc = created_at.with_timezone(&chrono::Utc);
            let session_age_hours = now.signed_duration_since(created_at_utc).num_hours();

            if session_age_hours >= session_time_threshold as i64 {
                info!(
                    "Session {} meets time threshold: {}h >= {}h",
                    session.session_id, session_age_hours, session_time_threshold
                );
                return true;
            }

            // 检查消息数量和时间的组合条件
            let message_count_ratio =
                session.context_length as f32 / message_count_threshold as f32;
            let time_ratio = session_age_hours as f32 / session_time_threshold as f32;

            if message_count_ratio + time_ratio >= 1.0 {
                info!(
                    "Session {} meets combined threshold: message_ratio={:.2}, time_ratio={:.2}",
                    session.session_id, message_count_ratio, time_ratio
                );
                return true;
            }
        } else {
            warn!("Failed to parse session created_at: {}", session.created_at);
        }

        false
    }

    /// 手动触发转移（用于测试或立即执行）
    #[instrument]
    pub async fn manual_transfer(
        session_id: &str,
        message_count_threshold: i32,
    ) -> Result<Vec<String>, AppError> {
        info!("Manual transfer triggered for session: {}", session_id);

        MemoryStorageService::auto_transfer_stm_to_ltm(session_id, message_count_threshold).await
    }
}

/// 全局自动转移服务实例
static MEMORY_TRANSFER_SERVICE: once_cell::sync::OnceCell<MemoryTransferService> =
    once_cell::sync::OnceCell::new();

/// 初始化自动转移服务
pub async fn init_transfer_service(
    check_interval: u64,
    message_count_threshold: i32,
    session_time_threshold: i32,
) -> Result<(), AppError> {
    let mut service = MemoryTransferService::new(
        check_interval,
        message_count_threshold,
        session_time_threshold,
    );
    service.start().await?;

    MEMORY_TRANSFER_SERVICE
        .set(service)
        .map_err(|_| AppError::Internal("Transfer service already initialized".to_string()))?;

    Ok(())
}

/// 获取自动转移服务实例
pub fn get_transfer_service() -> Option<&'static MemoryTransferService> {
    MEMORY_TRANSFER_SERVICE.get()
}
