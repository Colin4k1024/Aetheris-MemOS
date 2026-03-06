//! Agent runtime abstraction: observe → decide → act.
//!
//! Core services (analyzer, predictor, scheduler) implement this trait so the system
//! can be described as an agent-based adaptive memory manager. Implementations may
//! remain rule-based; the abstraction allows future LLM-driven or pluggable agents.

#![allow(dead_code)]

use crate::models::*;

/// Context passed into the agent (input for observe).
#[derive(Debug, Clone)]
pub struct TaskContextBundle {
    pub task_context: TaskContext,
    pub resource_constraints: ResourceConstraints,
    pub preferences: TaskPreferences,
}

/// Observation produced by the analyzer agent: task characteristics and memory strategy.
#[derive(Debug, Clone)]
pub struct AnalyzerObservation {
    pub characteristics: TaskCharacteristics,
    pub memory_strategy: MemoryStrategy,
    pub confidence_score: f64,
}

/// Decision/action of the analyzer (same as observation for this agent).
pub type AnalyzerDecision = AnalyzerObservation;
pub type AnalyzerAction = AnalyzerObservation;

/// Observation for the predictor: the memory config to evaluate.
pub type PredictorObservation = MemoryConfig;

/// Decision of the predictor: prediction plus synergy and decay factors.
#[derive(Debug, Clone)]
pub struct PredictorDecision {
    pub prediction: PerformancePrediction,
    pub synergy_factor: f64,
    pub decay_factor: f64,
    pub performance_breakdown: PerformanceBreakdown,
}

/// Action of the scheduler: final selection result.
pub type SchedulerAction = super::scheduler::MemorySelectionResult;

/// Agent trait: observe context → decide from observation → act to produce outcome.
pub trait MemoryAgent {
    type Context;
    type Observation;
    type Decision;
    type Action;

    fn observe(&self, context: &Self::Context) -> impl std::future::Future<Output = Self::Observation> + Send;
    fn decide(&self, observation: &Self::Observation) -> impl std::future::Future<Output = Self::Decision> + Send;
    fn act(&self, decision: &Self::Decision) -> impl std::future::Future<Output = Result<Self::Action, crate::AppError>> + Send;
}
