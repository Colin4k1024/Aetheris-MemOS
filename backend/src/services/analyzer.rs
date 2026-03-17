use crate::models::*;
use crate::services::agent::{AnalyzerAction, AnalyzerDecision, AnalyzerObservation, MemoryAgent};

pub struct TaskCharacteristicAnalyzer;

impl TaskCharacteristicAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze_task_characteristics(
        &self,
        task_context: &TaskContextInput,
    ) -> (TaskCharacteristics, MemoryStrategy, f64) {
        // 1. 复杂度评估
        let complexity = self.assess_task_complexity(task_context);

        // 2. 模态需求检测
        let modality_count = task_context.modality.len();

        // 3. 时间范围分析
        let temporal_scope = self.analyze_temporal_requirements(task_context);

        // 4. 推理深度评估
        let reasoning_depth = self.evaluate_reasoning_requirements(task_context);

        // 5. 上下文依赖度
        let context_dependency = self.measure_context_dependency(task_context);

        let characteristics = TaskCharacteristics {
            complexity,
            modality_count,
            temporal_scope,
            reasoning_depth,
            context_dependency,
        };

        // 6. 确定记忆策略
        let memory_strategy = self.determine_memory_strategy(&characteristics);

        // 7. 置信度评分
        let confidence_score = self.calculate_confidence_score(&characteristics);

        (characteristics, memory_strategy, confidence_score)
    }

    fn assess_task_complexity(&self, task_context: &TaskContextInput) -> f64 {
        let content_length = task_context.content.len() as f64;
        let history_length = task_context.context_history.len() as f64;

        // 基于内容长度和上下文历史长度评估复杂度
        let length_factor = (content_length / 1000.0).min(1.0);
        let history_factor = (history_length / 10.0).min(1.0);

        // 检查是否有复杂关键词
        let complex_keywords = ["分析", "推理", "计算", "优化", "设计", "评估"];
        let keyword_factor = if complex_keywords
            .iter()
            .any(|kw| task_context.content.contains(*kw))
        {
            0.3
        } else {
            0.0
        };

        // 综合复杂度评分
        (length_factor * 0.4 + history_factor * 0.3 + keyword_factor).min(1.0)
    }

    fn analyze_temporal_requirements(&self, task_context: &TaskContextInput) -> String {
        if let Some(metadata) = &task_context.task_metadata {
            if let Some(duration) = &metadata.expected_duration {
                return duration.clone();
            }
        }

        // 基于上下文历史长度推断时间范围
        let history_length = task_context.context_history.len();
        if history_length > 20 {
            "long".to_string()
        } else if history_length > 5 {
            "medium".to_string()
        } else {
            "short".to_string()
        }
    }

    fn evaluate_reasoning_requirements(&self, task_context: &TaskContextInput) -> f64 {
        let complexity = self.assess_task_complexity(task_context);

        // 推理关键词
        let reasoning_keywords = ["为什么", "如何", "分析", "推理", "原因", "逻辑"];
        let has_reasoning = reasoning_keywords
            .iter()
            .any(|kw| task_context.content.contains(*kw));

        if has_reasoning {
            (complexity * 1.2).min(1.0)
        } else {
            complexity * 0.5
        }
    }

    fn measure_context_dependency(&self, task_context: &TaskContextInput) -> f64 {
        let history_length = task_context.context_history.len() as f64;
        (history_length / 20.0).min(1.0)
    }

    fn determine_memory_strategy(&self, characteristics: &TaskCharacteristics) -> MemoryStrategy {
        let mut strategy = MemoryStrategy {
            primary_memory: "stm".to_string(),
            secondary_memory: Vec::new(),
            enable_multimodal: false,
            reasoning_depth: "shallow".to_string(),
        };

        // 复杂任务需要长期记忆
        if characteristics.complexity > 0.7 {
            strategy.secondary_memory.push("ltm".to_string());
        }

        // 多模态任务启用多模态记忆
        if characteristics.modality_count > 1 {
            strategy.enable_multimodal = true;
            strategy.secondary_memory.push("mm".to_string());
        }

        // 深度推理任务需要知识图谱
        if characteristics.reasoning_depth > 0.8 {
            strategy.secondary_memory.push("kg".to_string());
            strategy.reasoning_depth = "deep".to_string();
        } else if characteristics.reasoning_depth > 0.5 {
            strategy.reasoning_depth = "medium".to_string();
        }

        strategy
    }

    fn calculate_confidence_score(&self, characteristics: &TaskCharacteristics) -> f64 {
        // 基于特征完整性和一致性计算置信度
        let mut score = 0.5;

        // 复杂度评估置信度
        if characteristics.complexity > 0.0 && characteristics.complexity < 1.0 {
            score += 0.2;
        }

        // 模态数量合理性
        if characteristics.modality_count <= 4 {
            score += 0.15;
        }

        // 推理深度合理性
        if characteristics.reasoning_depth >= 0.0 && characteristics.reasoning_depth <= 1.0 {
            score += 0.15;
        }

        (score as f64).min(1.0)
    }
}

impl MemoryAgent for TaskCharacteristicAnalyzer {
    type Context = TaskContextInput;
    type Observation = AnalyzerObservation;
    type Decision = AnalyzerDecision;
    type Action = AnalyzerAction;

    fn observe(
        &self,
        context: &Self::Context,
    ) -> impl std::future::Future<Output = Self::Observation> + Send {
        let (characteristics, memory_strategy, confidence_score) =
            self.analyze_task_characteristics(context);
        std::future::ready(AnalyzerObservation {
            characteristics,
            memory_strategy,
            confidence_score,
        })
    }

    fn decide(
        &self,
        observation: &Self::Observation,
    ) -> impl std::future::Future<Output = Self::Decision> + Send {
        std::future::ready(observation.clone())
    }

    fn act(
        &self,
        decision: &Self::Decision,
    ) -> impl std::future::Future<Output = Result<Self::Action, crate::AppError>> + Send {
        std::future::ready(Ok(decision.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assess_task_complexity() {
        let analyzer = TaskCharacteristicAnalyzer::new();
        let task = TaskContextInput {
            content: "这是一个简单的任务".to_string(),
            modality: vec!["text".to_string()],
            context_history: Vec::new(),
            task_metadata: None,
        };

        let complexity = analyzer.assess_task_complexity(&task);
        assert!(complexity >= 0.0 && complexity <= 1.0);
    }

    #[test]
    fn test_determine_memory_strategy() {
        let analyzer = TaskCharacteristicAnalyzer::new();
        let characteristics = TaskCharacteristics {
            complexity: 0.8,
            modality_count: 2,
            temporal_scope: "medium".to_string(),
            reasoning_depth: 0.9,
            context_dependency: 0.6,
        };

        let strategy = analyzer.determine_memory_strategy(&characteristics);
        assert_eq!(strategy.primary_memory, "stm");
        assert!(strategy.secondary_memory.contains(&"ltm".to_string()));
        assert!(strategy.secondary_memory.contains(&"kg".to_string()));
        assert!(strategy.enable_multimodal);
    }
}
