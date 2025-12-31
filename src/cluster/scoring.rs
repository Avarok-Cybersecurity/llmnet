//! Node scoring algorithm for intelligent scheduling
//!
//! This module calculates a composite score for each node based on its
//! current resource utilization. Higher scores indicate more available
//! resources and thus more suitable for scheduling new pipeline replicas.

use chrono::Utc;

use super::node::{NodeMetrics, NodeScore, ScoreBreakdown};

/// Weight configuration for node scoring
///
/// Each weight determines how much that resource contributes to the
/// overall node score. Weights should sum to approximately 1.0.
#[derive(Debug, Clone)]
pub struct ScoringWeights {
    /// Weight for CPU availability (default: 0.20)
    pub cpu: f64,
    /// Weight for memory availability (default: 0.25)
    pub memory: f64,
    /// Weight for GPU availability (default: 0.30, redistributed if no GPU)
    pub gpu: f64,
    /// Weight for disk availability (default: 0.10)
    pub disk: f64,
    /// Weight for request load (default: 0.15)
    pub load: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            cpu: 0.20,
            memory: 0.25,
            gpu: 0.30,
            disk: 0.10,
            load: 0.15,
        }
    }
}

impl ScoringWeights {
    /// Create weights with custom values
    pub fn new(cpu: f64, memory: f64, gpu: f64, disk: f64, load: f64) -> Self {
        Self {
            cpu,
            memory,
            gpu,
            disk,
            load,
        }
    }

    /// Create weights optimized for GPU-heavy workloads
    pub fn gpu_heavy() -> Self {
        Self {
            cpu: 0.10,
            memory: 0.15,
            gpu: 0.50,
            disk: 0.05,
            load: 0.20,
        }
    }

    /// Create weights optimized for CPU-heavy workloads
    pub fn cpu_heavy() -> Self {
        Self {
            cpu: 0.40,
            memory: 0.25,
            gpu: 0.10,
            disk: 0.10,
            load: 0.15,
        }
    }

    /// Redistribute GPU weight when node has no GPU
    fn redistribute_for_no_gpu(&self) -> Self {
        let redistribution = self.gpu / 4.0;
        Self {
            cpu: self.cpu + redistribution,
            memory: self.memory + redistribution,
            gpu: 0.0,
            disk: self.disk + redistribution,
            load: self.load + redistribution,
        }
    }
}

/// Calculate a node score from its current metrics
///
/// The score ranges from 0.0 to 100.0, where higher is better.
/// - 100: Node has no load (all resources available)
/// - 0: Node is fully utilized
///
/// # Arguments
/// * `metrics` - Current metrics from the node
/// * `has_gpu` - Whether this node has GPU capability
/// * `weights` - Optional custom weights (uses defaults if None)
///
/// # Returns
/// A `NodeScore` containing the overall score and breakdown by component
pub fn calculate_node_score(
    metrics: &NodeMetrics,
    has_gpu: bool,
    weights: Option<&ScoringWeights>,
) -> NodeScore {
    let default_weights = ScoringWeights::default();
    let weights = weights.unwrap_or(&default_weights);

    // Convert usage percentages to availability scores (100 - usage)
    let cpu_score = (100.0 - metrics.cpu_usage_percent).max(0.0);
    let memory_score = (100.0 - metrics.memory_usage_percent).max(0.0);
    let disk_score = (100.0 - metrics.disk_usage_percent).max(0.0);

    // Load score: inverse of active requests, normalized
    // More concurrent requests = lower score
    // Formula: 100 / (1 + active_requests * 0.1)
    // With 0 requests: 100, with 10 requests: ~50, with 100 requests: ~9
    let load_score = 100.0 / (1.0 + metrics.active_requests as f64 * 0.1);

    // Handle GPU scoring
    let (gpu_score, adjusted_weights) = if has_gpu {
        let gpu_score = metrics
            .gpu_usage_percent
            .map(|g| (100.0 - g).max(0.0))
            .unwrap_or(100.0); // Assume available if no reading
        (Some(gpu_score), weights.clone())
    } else {
        // Redistribute GPU weight to other metrics
        (None, weights.redistribute_for_no_gpu())
    };

    // Calculate weighted total
    let mut total = cpu_score * adjusted_weights.cpu
        + memory_score * adjusted_weights.memory
        + disk_score * adjusted_weights.disk
        + load_score * adjusted_weights.load;

    if let Some(gs) = gpu_score {
        total += gs * adjusted_weights.gpu;
    }

    NodeScore {
        score: total.clamp(0.0, 100.0),
        breakdown: ScoreBreakdown {
            cpu_score,
            memory_score,
            gpu_score,
            disk_score,
            load_score,
        },
        calculated_at: Utc::now(),
    }
}

/// Compare two nodes and return which is preferred for scheduling
///
/// Returns `Ordering::Greater` if `a` is better, `Ordering::Less` if `b` is better.
pub fn compare_node_scores(a: &NodeScore, b: &NodeScore) -> std::cmp::Ordering {
    a.score
        .partial_cmp(&b.score)
        .unwrap_or(std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(cpu: f64, memory: f64, disk: f64, active: u32) -> NodeMetrics {
        NodeMetrics {
            cpu_usage_percent: cpu,
            memory_usage_percent: memory,
            gpu_usage_percent: None,
            gpu_memory_usage_percent: None,
            disk_usage_percent: disk,
            request_count: 0,
            avg_latency_ms: 0.0,
            active_requests: active,
            collected_at: Utc::now(),
        }
    }

    #[test]
    fn test_idle_node_high_score() {
        let metrics = make_metrics(0.0, 0.0, 0.0, 0);
        let score = calculate_node_score(&metrics, false, None);

        // Idle node should have score close to 100
        assert!(
            score.score > 95.0,
            "Idle node score should be > 95, got {}",
            score.score
        );
    }

    #[test]
    fn test_busy_node_low_score() {
        let metrics = make_metrics(90.0, 90.0, 90.0, 100);
        let score = calculate_node_score(&metrics, false, None);

        // Busy node should have score close to 0
        assert!(
            score.score < 20.0,
            "Busy node score should be < 20, got {}",
            score.score
        );
    }

    #[test]
    fn test_moderate_load() {
        let metrics = make_metrics(50.0, 50.0, 50.0, 5);
        let score = calculate_node_score(&metrics, false, None);

        // Moderate load should be around 50
        assert!(
            score.score > 40.0 && score.score < 60.0,
            "Moderate load score should be 40-60, got {}",
            score.score
        );
    }

    #[test]
    fn test_gpu_node_scoring() {
        let mut metrics = make_metrics(50.0, 50.0, 50.0, 5);
        metrics.gpu_usage_percent = Some(30.0);
        metrics.gpu_memory_usage_percent = Some(40.0);

        let score = calculate_node_score(&metrics, true, None);

        // GPU node with low GPU usage should score higher
        assert!(score.breakdown.gpu_score.is_some());
        assert_eq!(score.breakdown.gpu_score.unwrap(), 70.0);
    }

    #[test]
    fn test_score_comparison() {
        let metrics_idle = make_metrics(10.0, 10.0, 10.0, 1);
        let metrics_busy = make_metrics(80.0, 80.0, 80.0, 50);

        let score_idle = calculate_node_score(&metrics_idle, false, None);
        let score_busy = calculate_node_score(&metrics_busy, false, None);

        // Idle node should be preferred
        assert_eq!(
            compare_node_scores(&score_idle, &score_busy),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_custom_weights() {
        let metrics = make_metrics(50.0, 50.0, 50.0, 5);
        let cpu_heavy = ScoringWeights::cpu_heavy();

        let score_default = calculate_node_score(&metrics, false, None);
        let score_cpu = calculate_node_score(&metrics, false, Some(&cpu_heavy));

        // Both should be valid scores
        assert!(score_default.score > 0.0);
        assert!(score_cpu.score > 0.0);
    }

    #[test]
    fn test_breakdown_components() {
        let metrics = make_metrics(30.0, 40.0, 50.0, 10);
        let score = calculate_node_score(&metrics, false, None);

        assert_eq!(score.breakdown.cpu_score, 70.0);
        assert_eq!(score.breakdown.memory_score, 60.0);
        assert_eq!(score.breakdown.disk_score, 50.0);
        assert!(score.breakdown.load_score > 0.0);
        assert!(score.breakdown.gpu_score.is_none());
    }

    #[test]
    fn test_score_clamping() {
        // Test with values that could exceed bounds
        let metrics = NodeMetrics {
            cpu_usage_percent: 150.0,    // Invalid but should be handled
            memory_usage_percent: -10.0, // Invalid but should be handled
            gpu_usage_percent: None,
            gpu_memory_usage_percent: None,
            disk_usage_percent: 50.0,
            request_count: 0,
            avg_latency_ms: 0.0,
            active_requests: 0,
            collected_at: Utc::now(),
        };

        let score = calculate_node_score(&metrics, false, None);

        // Score should be clamped to 0-100
        assert!(score.score >= 0.0 && score.score <= 100.0);
    }
}
