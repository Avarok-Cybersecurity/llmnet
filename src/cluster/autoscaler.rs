//! Horizontal Pod Auto-scaler for LLMNet pipelines
//!
//! This module implements automatic scaling of pipeline replicas based on
//! aggregate resource utilization across nodes running the pipeline.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::node::NodeMetrics;
use super::pipeline::AutoscalingConfig;

/// Auto-scaler state for a pipeline
///
/// Tracks the last scaling actions to enforce cooldown periods
/// and prevent thrashing (rapid scale up/down cycles).
#[derive(Debug, Clone, Default)]
pub struct AutoscalerState {
    /// Last scale-up time
    pub last_scale_up: Option<DateTime<Utc>>,
    /// Last scale-down time
    pub last_scale_down: Option<DateTime<Utc>>,
    /// Historical metrics for smoothing (future use)
    pub metric_history: Vec<AggregateMetrics>,
}

impl AutoscalerState {
    /// Create a new auto-scaler state
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a scale-up action
    pub fn record_scale_up(&mut self) {
        self.last_scale_up = Some(Utc::now());
    }

    /// Record a scale-down action
    pub fn record_scale_down(&mut self) {
        self.last_scale_down = Some(Utc::now());
    }

    /// Check if scale-up is allowed (cooldown has passed)
    pub fn can_scale_up(&self, cooldown_seconds: u64) -> bool {
        self.last_scale_up
            .map(|t| (Utc::now() - t).num_seconds() >= cooldown_seconds as i64)
            .unwrap_or(true)
    }

    /// Check if scale-down is allowed (cooldown has passed)
    pub fn can_scale_down(&self, cooldown_seconds: u64) -> bool {
        self.last_scale_down
            .map(|t| (Utc::now() - t).num_seconds() >= cooldown_seconds as i64)
            .unwrap_or(true)
    }
}

/// Aggregate metrics across all nodes running a pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateMetrics {
    /// Average CPU utilization across nodes
    #[serde(rename = "avgCpuUsage")]
    pub avg_cpu_usage: f64,

    /// Average memory utilization across nodes
    #[serde(rename = "avgMemoryUsage")]
    pub avg_memory_usage: f64,

    /// Total requests across all nodes
    #[serde(rename = "totalRequestCount")]
    pub total_request_count: u64,

    /// Average latency across nodes
    #[serde(rename = "avgLatencyMs")]
    pub avg_latency_ms: f64,

    /// Total active requests across nodes
    #[serde(rename = "totalActiveRequests")]
    pub total_active_requests: u32,

    /// Number of nodes included in this aggregation
    #[serde(rename = "nodeCount")]
    pub node_count: usize,

    /// When these metrics were collected
    #[serde(rename = "collectedAt")]
    pub collected_at: DateTime<Utc>,
}

impl Default for AggregateMetrics {
    fn default() -> Self {
        Self {
            avg_cpu_usage: 0.0,
            avg_memory_usage: 0.0,
            total_request_count: 0,
            avg_latency_ms: 0.0,
            total_active_requests: 0,
            node_count: 0,
            collected_at: Utc::now(),
        }
    }
}

/// Scaling decision from the auto-scaler
#[derive(Debug, Clone, PartialEq)]
pub enum ScalingDecision {
    /// No change needed
    NoChange,
    /// Scale up to the target number of replicas
    ScaleUp {
        target_replicas: u32,
        reason: String,
    },
    /// Scale down to the target number of replicas
    ScaleDown {
        target_replicas: u32,
        reason: String,
    },
}

/// Calculate aggregate metrics for a pipeline across nodes
///
/// Takes a list of (node_name, metrics) tuples and computes averages.
pub fn aggregate_pipeline_metrics(node_metrics: &[(String, NodeMetrics)]) -> AggregateMetrics {
    if node_metrics.is_empty() {
        return AggregateMetrics::default();
    }

    let n = node_metrics.len() as f64;

    let avg_cpu = node_metrics
        .iter()
        .map(|(_, m)| m.cpu_usage_percent)
        .sum::<f64>()
        / n;

    let avg_memory = node_metrics
        .iter()
        .map(|(_, m)| m.memory_usage_percent)
        .sum::<f64>()
        / n;

    let total_requests: u64 = node_metrics.iter().map(|(_, m)| m.request_count).sum();

    let avg_latency = node_metrics
        .iter()
        .map(|(_, m)| m.avg_latency_ms)
        .sum::<f64>()
        / n;

    let total_active: u32 = node_metrics.iter().map(|(_, m)| m.active_requests).sum();

    AggregateMetrics {
        avg_cpu_usage: avg_cpu,
        avg_memory_usage: avg_memory,
        total_request_count: total_requests,
        avg_latency_ms: avg_latency,
        total_active_requests: total_active,
        node_count: node_metrics.len(),
        collected_at: Utc::now(),
    }
}

/// Evaluate scaling decision for a pipeline
///
/// Determines whether to scale up, scale down, or maintain current replicas
/// based on aggregate metrics and the auto-scaling configuration.
pub fn evaluate_scaling(
    config: &AutoscalingConfig,
    current_replicas: u32,
    aggregate: &AggregateMetrics,
    state: &AutoscalerState,
) -> ScalingDecision {
    // If no nodes are reporting, don't scale
    if aggregate.node_count == 0 {
        return ScalingDecision::NoChange;
    }

    // Check cooldowns
    let can_scale_up = state.can_scale_up(config.scale_up_cooldown_seconds);
    let can_scale_down = state.can_scale_down(config.scale_down_cooldown_seconds);

    // Calculate desired replicas based on CPU utilization
    // Formula: desired = ceil(current * actual_usage / target_usage)
    let cpu_desired = if config.target_cpu_utilization > 0.0 {
        ((current_replicas as f64 * aggregate.avg_cpu_usage) / config.target_cpu_utilization).ceil()
            as u32
    } else {
        current_replicas
    };

    // Calculate desired replicas based on memory utilization
    let memory_desired = if config.target_memory_utilization > 0.0 {
        ((current_replicas as f64 * aggregate.avg_memory_usage)
            / config.target_memory_utilization)
            .ceil() as u32
    } else {
        current_replicas
    };

    // Take the maximum (most conservative - more replicas)
    let desired = cpu_desired.max(memory_desired);

    // Clamp to min/max bounds
    let clamped = desired.clamp(config.min_replicas, config.max_replicas);

    // Determine action
    if clamped > current_replicas && can_scale_up {
        // Scale up
        let delta = (clamped - current_replicas).min(config.behavior.max_scale_up);
        let target = current_replicas + delta;

        let reason = format!(
            "High utilization: CPU {:.1}% (target: {:.1}%), Memory {:.1}% (target: {:.1}%)",
            aggregate.avg_cpu_usage,
            config.target_cpu_utilization,
            aggregate.avg_memory_usage,
            config.target_memory_utilization
        );

        ScalingDecision::ScaleUp {
            target_replicas: target.min(config.max_replicas),
            reason,
        }
    } else if clamped < current_replicas && can_scale_down {
        // Scale down
        let delta = (current_replicas - clamped).min(config.behavior.max_scale_down);
        let target = current_replicas - delta;

        let reason = format!(
            "Low utilization: CPU {:.1}% (target: {:.1}%), Memory {:.1}% (target: {:.1}%)",
            aggregate.avg_cpu_usage,
            config.target_cpu_utilization,
            aggregate.avg_memory_usage,
            config.target_memory_utilization
        );

        ScalingDecision::ScaleDown {
            target_replicas: target.max(config.min_replicas),
            reason,
        }
    } else {
        ScalingDecision::NoChange
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> AutoscalingConfig {
        AutoscalingConfig {
            min_replicas: 1,
            max_replicas: 10,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            scale_up_cooldown_seconds: 60,
            scale_down_cooldown_seconds: 300,
            behavior: super::super::pipeline::ScalingBehavior {
                max_scale_up: 2,
                max_scale_down: 2,
            },
        }
    }

    fn make_metrics(cpu: f64, memory: f64) -> AggregateMetrics {
        AggregateMetrics {
            avg_cpu_usage: cpu,
            avg_memory_usage: memory,
            total_request_count: 100,
            avg_latency_ms: 50.0,
            total_active_requests: 5,
            node_count: 1,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn test_no_change_at_target() {
        let config = make_config();
        let metrics = make_metrics(70.0, 80.0); // At target
        let state = AutoscalerState::new();

        let decision = evaluate_scaling(&config, 3, &metrics, &state);
        assert_eq!(decision, ScalingDecision::NoChange);
    }

    #[test]
    fn test_scale_up_high_cpu() {
        let config = make_config();
        let metrics = make_metrics(90.0, 50.0); // High CPU
        let state = AutoscalerState::new();

        let decision = evaluate_scaling(&config, 3, &metrics, &state);
        match decision {
            ScalingDecision::ScaleUp { target_replicas, .. } => {
                assert!(target_replicas > 3);
                assert!(target_replicas <= 5); // max_scale_up is 2
            }
            _ => panic!("Expected ScaleUp, got {:?}", decision),
        }
    }

    #[test]
    fn test_scale_down_low_usage() {
        let config = make_config();
        let metrics = make_metrics(20.0, 30.0); // Low usage
        let state = AutoscalerState::new();

        let decision = evaluate_scaling(&config, 5, &metrics, &state);
        match decision {
            ScalingDecision::ScaleDown { target_replicas, .. } => {
                assert!(target_replicas < 5);
                assert!(target_replicas >= 3); // max_scale_down is 2
            }
            _ => panic!("Expected ScaleDown, got {:?}", decision),
        }
    }

    #[test]
    fn test_respects_min_replicas() {
        let config = make_config();
        let metrics = make_metrics(10.0, 10.0); // Very low usage
        let state = AutoscalerState::new();

        let decision = evaluate_scaling(&config, 2, &metrics, &state);
        match decision {
            ScalingDecision::ScaleDown { target_replicas, .. } => {
                assert!(target_replicas >= config.min_replicas);
            }
            ScalingDecision::NoChange => {} // Also acceptable if already at min
            _ => panic!("Expected ScaleDown or NoChange, got {:?}", decision),
        }
    }

    #[test]
    fn test_respects_max_replicas() {
        let config = make_config();
        let metrics = make_metrics(95.0, 95.0); // Very high usage
        let state = AutoscalerState::new();

        let decision = evaluate_scaling(&config, 9, &metrics, &state);
        match decision {
            ScalingDecision::ScaleUp { target_replicas, .. } => {
                assert!(target_replicas <= config.max_replicas);
            }
            ScalingDecision::NoChange => {} // Also acceptable if at max
            _ => panic!("Expected ScaleUp or NoChange, got {:?}", decision),
        }
    }

    #[test]
    fn test_cooldown_prevents_scale_up() {
        let config = make_config();
        let metrics = make_metrics(90.0, 50.0);
        let mut state = AutoscalerState::new();
        state.last_scale_up = Some(Utc::now()); // Just scaled up

        let decision = evaluate_scaling(&config, 3, &metrics, &state);
        assert_eq!(decision, ScalingDecision::NoChange);
    }

    #[test]
    fn test_cooldown_prevents_scale_down() {
        let config = make_config();
        let metrics = make_metrics(20.0, 20.0);
        let mut state = AutoscalerState::new();
        state.last_scale_down = Some(Utc::now()); // Just scaled down

        let decision = evaluate_scaling(&config, 5, &metrics, &state);
        assert_eq!(decision, ScalingDecision::NoChange);
    }

    #[test]
    fn test_aggregate_metrics() {
        let node_metrics = vec![
            (
                "node1".to_string(),
                NodeMetrics {
                    cpu_usage_percent: 60.0,
                    memory_usage_percent: 70.0,
                    gpu_usage_percent: None,
                    gpu_memory_usage_percent: None,
                    disk_usage_percent: 50.0,
                    request_count: 100,
                    avg_latency_ms: 50.0,
                    active_requests: 5,
                    collected_at: Utc::now(),
                },
            ),
            (
                "node2".to_string(),
                NodeMetrics {
                    cpu_usage_percent: 80.0,
                    memory_usage_percent: 90.0,
                    gpu_usage_percent: None,
                    gpu_memory_usage_percent: None,
                    disk_usage_percent: 60.0,
                    request_count: 200,
                    avg_latency_ms: 100.0,
                    active_requests: 10,
                    collected_at: Utc::now(),
                },
            ),
        ];

        let aggregate = aggregate_pipeline_metrics(&node_metrics);

        assert_eq!(aggregate.node_count, 2);
        assert_eq!(aggregate.avg_cpu_usage, 70.0);
        assert_eq!(aggregate.avg_memory_usage, 80.0);
        assert_eq!(aggregate.total_request_count, 300);
        assert_eq!(aggregate.avg_latency_ms, 75.0);
        assert_eq!(aggregate.total_active_requests, 15);
    }

    #[test]
    fn test_empty_metrics() {
        let aggregate = aggregate_pipeline_metrics(&[]);
        assert_eq!(aggregate.node_count, 0);
        assert_eq!(aggregate.avg_cpu_usage, 0.0);
    }

    #[test]
    fn test_autoscaler_state_cooldowns() {
        let mut state = AutoscalerState::new();

        // Initially, can scale
        assert!(state.can_scale_up(60));
        assert!(state.can_scale_down(300));

        // After recording scale-up, can't immediately scale up again
        state.record_scale_up();
        assert!(!state.can_scale_up(60));
        assert!(state.can_scale_down(300)); // Can still scale down
    }
}
