//! Procedural Memory Data Model
//!
//! Defines the structure for "how-to-do" skill/process memories:
//! tool call chains, operation steps, and execution context.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralEntry {
    pub name: String,
    pub description: String,
    pub task_type: String,
    pub steps: Vec<ProceduralStep>,
    pub preconditions: Vec<String>,
    pub tools_used: Vec<String>,
    pub success_rate: f64,
    pub execution_count: u32,
    pub version: u32,
    pub context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProceduralStep {
    pub order: u32,
    pub action: String,
    pub tool: Option<String>,
    pub parameters: HashMap<String, serde_json::Value>,
    pub expected_output: Option<String>,
    pub fallback: Option<String>,
}

impl ProceduralEntry {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name is required".to_string());
        }
        if self.steps.is_empty() {
            return Err("at least one step is required".to_string());
        }
        for (i, step) in self.steps.iter().enumerate() {
            if step.action.is_empty() {
                return Err(format!("step {} action is required", i));
            }
        }
        let mut orders: Vec<u32> = self.steps.iter().map(|s| s.order).collect();
        orders.sort_unstable();
        orders.dedup();
        if orders.len() != self.steps.len() {
            return Err("step order values must be unique".to_string());
        }
        if !(0.0..=1.0).contains(&self.success_rate) {
            return Err("success_rate must be between 0.0 and 1.0".to_string());
        }
        Ok(())
    }

    pub fn tools_summary(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter_map(|s| s.tool.clone())
            .collect()
    }

    pub fn searchable_text(&self) -> String {
        let steps_text: String = self
            .steps
            .iter()
            .map(|s| s.action.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        format!("{} {} {}", self.name, self.description, steps_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry() -> ProceduralEntry {
        ProceduralEntry {
            name: "deploy-k8s".to_string(),
            description: "Deploy service to Kubernetes".to_string(),
            task_type: "deployment".to_string(),
            steps: vec![
                ProceduralStep {
                    order: 0,
                    action: "Build Docker image".to_string(),
                    tool: Some("docker".to_string()),
                    parameters: HashMap::new(),
                    expected_output: Some("Image built".to_string()),
                    fallback: None,
                },
                ProceduralStep {
                    order: 1,
                    action: "Push to registry".to_string(),
                    tool: Some("docker".to_string()),
                    parameters: HashMap::new(),
                    expected_output: None,
                    fallback: None,
                },
                ProceduralStep {
                    order: 2,
                    action: "Apply kubectl manifests".to_string(),
                    tool: Some("kubectl".to_string()),
                    parameters: HashMap::new(),
                    expected_output: None,
                    fallback: Some("Rollback deployment".to_string()),
                },
            ],
            preconditions: vec!["Docker running".to_string(), "kubectl configured".to_string()],
            tools_used: vec!["docker".to_string(), "kubectl".to_string()],
            success_rate: 0.85,
            execution_count: 12,
            version: 1,
            context: HashMap::new(),
        }
    }

    #[test]
    fn validates_correct_entry() {
        assert!(sample_entry().validate().is_ok());
    }

    #[test]
    fn rejects_empty_name() {
        let mut entry = sample_entry();
        entry.name = String::new();
        assert!(entry.validate().is_err());
    }

    #[test]
    fn rejects_empty_steps() {
        let mut entry = sample_entry();
        entry.steps = vec![];
        assert!(entry.validate().is_err());
    }

    #[test]
    fn rejects_invalid_success_rate() {
        let mut entry = sample_entry();
        entry.success_rate = 1.5;
        assert!(entry.validate().is_err());
    }

    #[test]
    fn searchable_text_includes_all_parts() {
        let entry = sample_entry();
        let text = entry.searchable_text();
        assert!(text.contains("deploy-k8s"));
        assert!(text.contains("Kubernetes"));
        assert!(text.contains("kubectl"));
    }

    #[test]
    fn serialization_roundtrip() {
        let entry = sample_entry();
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ProceduralEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, entry.name);
        assert_eq!(deserialized.steps.len(), 3);
    }
}
