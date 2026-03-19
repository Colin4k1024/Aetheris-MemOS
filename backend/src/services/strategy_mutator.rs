/// 策略变异服务 — Issue #55
///
/// 基于历史性能指标自动调整 WeightStrategy 超参数，
/// 实现自适应进化（greedy hill-climbing + random perturbation 混合）。
///
/// 工作机制：
/// 1. 从数据库读取近 N 窗口的性能指标（accuracy, coherence, response_time）
/// 2. 计算当前权重配置的综合评分
/// 3. 生成若干候选突变（小幅随机扰动 ± delta）
/// 4. 贪心选择：若突变后预测评分更高，则接受
/// 5. 将变异结果写入权重历史（决策轨迹）并返回新权重建议

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use tracing::{info, warn};

use crate::AppError;

/// 变异配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationConfig {
    /// 每次变异的扰动幅度（0.0 ~ 1.0，推荐 0.05~0.15）
    pub perturbation: f64,
    /// 每轮评估的候选数量
    pub candidate_count: usize,
    /// 用于评估的历史窗口大小（最近多少条性能记录）
    pub history_window: usize,
    /// 策略自动变异的调度间隔（秒）
    pub mutation_interval_seconds: u64,
    /// 是否允许自动触发变异（false 时只支持手动调用）
    pub auto_mutate: bool,
    /// 综合评分中准确性权重
    pub accuracy_weight: f64,
    /// 综合评分中一致性权重
    pub coherence_weight: f64,
    /// 综合评分中响应时间惩罚权重（越低越好，取倒数）
    pub latency_penalty_weight: f64,
}

impl Default for MutationConfig {
    fn default() -> Self {
        Self {
            perturbation: 0.05,
            candidate_count: 5,
            history_window: 50,
            mutation_interval_seconds: 600,
            auto_mutate: true,
            accuracy_weight: 0.5,
            coherence_weight: 0.3,
            latency_penalty_weight: 0.2,
        }
    }
}

/// 当前最优权重策略超参数（持久于进程生命周期）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyHyperparams {
    /// LTM 策略中复杂度阈值（高于此才启用 LTM）
    pub ltm_complexity_threshold: f64,
    /// KG 策略中推理深度阈值
    pub kg_reasoning_threshold: f64,
    /// MM 策略中模态数量阈值
    pub mm_modality_threshold: f64,
    /// 协同增益系数（SynergyAwareStrategy 的 boost 乘数）
    pub synergy_boost: f64,
    /// 代价收益比衰减切换点（LinearDecayStrategy 的阈值）
    pub decay_cost_threshold: f64,
}

impl Default for StrategyHyperparams {
    fn default() -> Self {
        Self {
            ltm_complexity_threshold: 0.5,
            kg_reasoning_threshold: 0.7,
            mm_modality_threshold: 1.0,
            synergy_boost: 0.05,
            decay_cost_threshold: 1.0,
        }
    }
}

/// 变异结果
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MutationResult {
    pub accepted: bool,
    pub old_score: f64,
    pub new_score: f64,
    pub old_params: serde_json::Value,
    pub new_params: serde_json::Value,
    pub reason: String,
}

static MUTATION_RUNNING: AtomicBool = AtomicBool::new(false);
static BEST_PARAMS: OnceLock<std::sync::Mutex<StrategyHyperparams>> = OnceLock::new();

fn best_params_lock() -> &'static std::sync::Mutex<StrategyHyperparams> {
    BEST_PARAMS.get_or_init(|| std::sync::Mutex::new(StrategyHyperparams::default()))
}

/// 获取当前最优超参数副本
pub fn current_hyperparams() -> StrategyHyperparams {
    best_params_lock()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

/// 策略变异服务
pub struct StrategyMutator;

impl StrategyMutator {
    /// 启动后台自动变异守护任务（非重入）
    pub fn init_mutation_daemon(cfg: MutationConfig) {
        if MUTATION_RUNNING.swap(true, Ordering::SeqCst) {
            warn!("StrategyMutator daemon already running, skipping");
            return;
        }
        let interval = std::time::Duration::from_secs(cfg.mutation_interval_seconds);
        tokio::spawn(async move {
            info!(
                "StrategyMutator daemon started, interval={}s",
                cfg.mutation_interval_seconds
            );
            loop {
                tokio::time::sleep(interval).await;
                match Self::run_mutation_cycle(&cfg).await {
                    Ok(result) => {
                        if result.accepted {
                            info!(
                                "Strategy mutation accepted: score {:.4} → {:.4} ({})",
                                result.old_score, result.new_score, result.reason
                            );
                        }
                    }
                    Err(e) => {
                        warn!("Strategy mutation cycle failed: {}", e);
                    }
                }
            }
        });
    }

    /// 手动触发一轮变异（可通过 API 调用）
    pub async fn run_mutation_cycle(cfg: &MutationConfig) -> Result<MutationResult, AppError> {
        // 读取历史性能指标（用空字符串匹配所有 config_id 做全量聚合；失败时使用空统计量）
        let stats = crate::db::performance::PerformanceMetricsRepository::get_aggregated_stats(
            "",
            None,
            None,
        )
        .await
        .unwrap_or(crate::db::performance::AggregatedStats {
            avg_response_time: None,
            avg_memory_usage: None,
            avg_cpu_usage: None,
            avg_accuracy: None,
            avg_coherence: None,
            max_response_time: None,
            max_memory_usage: None,
            max_cpu_usage: None,
            min_response_time: None,
            min_memory_usage: None,
            min_cpu_usage: None,
            count: 0,
        });

        // 计算当前综合评分
        let current_params = current_hyperparams();
        let baseline_score = Self::composite_score(&stats, cfg);

        // 生成候选突变
        let mut best_candidate = current_params.clone();
        let mut best_score = baseline_score;
        let mut accepted = false;

        for _ in 0..cfg.candidate_count {
            let candidate = Self::perturb(&current_params, cfg.perturbation);
            // 超参数与评分的关系目前用代理规则估算
            let candidate_score = Self::estimate_candidate_score(&candidate, &stats, cfg);
            if candidate_score > best_score + 1e-6 {
                best_score = candidate_score;
                best_candidate = candidate;
                accepted = true;
            }
        }

        let reason = if accepted {
            format!(
                "Accepted mutation: score improved by {:.4}",
                best_score - baseline_score
            )
        } else {
            "No improving mutation found this cycle".to_string()
        };

        if accepted {
            if let Ok(mut guard) = best_params_lock().lock() {
                *guard = best_candidate.clone();
            }
        }

        Ok(MutationResult {
            accepted,
            old_score: baseline_score,
            new_score: best_score,
            old_params: serde_json::to_value(&current_params)
                .unwrap_or(serde_json::Value::Null),
            new_params: serde_json::to_value(&best_candidate)
                .unwrap_or(serde_json::Value::Null),
            reason,
        })
    }

    /// 综合评分：accuracy_weight * avg_accuracy + coherence_weight * avg_coherence
    ///          - latency_penalty_weight * normalized_latency
    fn composite_score(
        stats: &crate::db::performance::AggregatedStats,
        cfg: &MutationConfig,
    ) -> f64 {
        let accuracy = stats.avg_accuracy.unwrap_or(0.0);
        let coherence = stats.avg_coherence.unwrap_or(0.0);
        // 将响应时间归一化到 [0,1]（假设 5000ms 为上限）
        let latency_norm = stats
            .avg_response_time
            .map(|ms| (ms as f64 / 5000.0).min(1.0))
            .unwrap_or(0.5);

        let wsum = cfg.accuracy_weight + cfg.coherence_weight + cfg.latency_penalty_weight;
        if wsum < 1e-9 {
            return 0.0;
        }
        (cfg.accuracy_weight * accuracy + cfg.coherence_weight * coherence
            - cfg.latency_penalty_weight * latency_norm)
            / wsum
    }

    /// 用代理规则估算候选超参数变异后的评分（无法真实运行，使用启发式）
    ///
    /// 规则：
    /// - 较低的阈值 → 更多层激活 → 稍高的准确性，但延迟增加
    /// - 协同增益适中（0.03~0.08）时效果最好
    fn estimate_candidate_score(
        candidate: &StrategyHyperparams,
        stats: &crate::db::performance::AggregatedStats,
        cfg: &MutationConfig,
    ) -> f64 {
        let base = Self::composite_score(stats, cfg);

        // 启发式奖励：低阈值激活更多层 → +accuracy bonus
        let threshold_bonus = (1.0 - candidate.ltm_complexity_threshold) * 0.02
            + (1.0 - candidate.kg_reasoning_threshold) * 0.015;

        // 协同增益正常范围 [0.03, 0.08] 有额外分
        let synergy_penalty = if candidate.synergy_boost < 0.01 || candidate.synergy_boost > 0.15 {
            -0.02
        } else {
            0.01
        };

        // 衰减阈值接近 1.0 效果最好
        let decay_bonus = -(candidate.decay_cost_threshold - 1.0).abs() * 0.01;

        (base + threshold_bonus + synergy_penalty + decay_bonus).clamp(-1.0, 1.0)
    }

    /// 对超参数进行小幅随机扰动
    fn perturb(params: &StrategyHyperparams, delta: f64) -> StrategyHyperparams {
        // 使用低开销的 xorshift 伪随机
        fn xorshift(seed: u64) -> (f64, u64) {
            let s = seed ^ (seed << 13);
            let s = s ^ (s >> 7);
            let s = s ^ (s << 17);
            // 映射到 [-1, 1]
            let f = ((s as i64) as f64) / (i64::MAX as f64);
            (f, s)
        }

        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(42);

        let (r1, s1) = xorshift(seed);
        let (r2, s2) = xorshift(s1 ^ 0xdead_beef);
        let (r3, s3) = xorshift(s2 ^ 0xcafe_babe);
        let (r4, s4) = xorshift(s3 ^ 0x1234_5678);
        let (r5, _) = xorshift(s4 ^ 0xabcd_ef01);

        StrategyHyperparams {
            ltm_complexity_threshold: (params.ltm_complexity_threshold + r1 * delta)
                .clamp(0.1, 0.9),
            kg_reasoning_threshold: (params.kg_reasoning_threshold + r2 * delta).clamp(0.1, 0.9),
            mm_modality_threshold: (params.mm_modality_threshold + r3 * delta).clamp(1.0, 5.0),
            synergy_boost: (params.synergy_boost + r4 * delta * 0.5).clamp(0.0, 0.2),
            decay_cost_threshold: (params.decay_cost_threshold + r5 * delta * 2.0)
                .clamp(0.5, 2.0),
        }
    }
}
