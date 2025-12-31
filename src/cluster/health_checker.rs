//! Active Health Checker for cluster deployments
//!
//! This module provides active HTTP health probing for deployed pipelines.
//! It runs as part of the orchestrator loop and updates replica health status
//! by probing each replica's health endpoint.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{debug, trace, warn};

use super::controller::ClusterController;
use super::node::ReplicaStatus;

/// Configuration for the health checker
#[derive(Debug, Clone)]
pub struct HealthCheckerConfig {
    /// Timeout for health check requests
    pub timeout_secs: u64,
    /// Number of consecutive failures before marking unhealthy
    pub failure_threshold: u32,
    /// Number of consecutive successes before marking healthy
    pub success_threshold: u32,
    /// Health endpoint path
    pub health_path: String,
}

impl Default for HealthCheckerConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 5,
            failure_threshold: 3,
            success_threshold: 1,
            health_path: "/health".to_string(),
        }
    }
}

/// Result of a single health probe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthProbeResult {
    /// Whether the probe succeeded
    pub success: bool,
    /// HTTP status code (if request completed)
    pub status_code: Option<u16>,
    /// Response time in milliseconds
    pub latency_ms: u64,
    /// Timestamp of the probe
    pub timestamp: DateTime<Utc>,
    /// Error message if probe failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Health state for a replica, stored in the controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaHealthState {
    /// Unique key: "node_name:namespace:pipeline_name:port"
    pub key: String,
    /// Node this replica runs on
    pub node_name: String,
    /// Pipeline namespace
    pub namespace: String,
    /// Pipeline name
    pub pipeline_name: String,
    /// Port the replica serves on
    pub port: u16,
    /// Full endpoint URL
    pub endpoint: String,
    /// Current health status
    pub status: ReplicaStatus,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Consecutive successes
    pub consecutive_successes: u32,
    /// Last probe result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_probe: Option<HealthProbeResult>,
    /// When the replica was first seen
    pub first_seen: DateTime<Utc>,
    /// When the replica became ready (if ever)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready_since: Option<DateTime<Utc>>,
}

impl ReplicaHealthState {
    /// Create a new health state for a replica
    pub fn new(
        node_name: &str,
        node_address: &str,
        namespace: &str,
        pipeline_name: &str,
        port: u16,
    ) -> Self {
        let endpoint = format!("http://{}:{}", node_address, port);
        let key = format!("{}:{}:{}:{}", node_name, namespace, pipeline_name, port);

        Self {
            key,
            node_name: node_name.to_string(),
            namespace: namespace.to_string(),
            pipeline_name: pipeline_name.to_string(),
            port,
            endpoint,
            status: ReplicaStatus::Starting,
            consecutive_failures: 0,
            consecutive_successes: 0,
            last_probe: None,
            first_seen: Utc::now(),
            ready_since: None,
        }
    }

    /// Calculate uptime if replica is running
    pub fn uptime(&self) -> Option<Duration> {
        self.ready_since.map(|t| {
            let now = Utc::now();
            (now - t).to_std().unwrap_or(Duration::ZERO)
        })
    }

    /// Format uptime as human-readable string
    pub fn uptime_str(&self) -> String {
        match self.uptime() {
            Some(d) => format_duration(d),
            None => "-".to_string(),
        }
    }
}

/// Format a duration as human-readable string
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d{}h", secs / 86400, (secs % 86400) / 3600)
    }
}

/// Probe a single endpoint's health
async fn probe_endpoint(
    client: &Client,
    endpoint: &str,
    health_path: &str,
    timeout_duration: Duration,
) -> HealthProbeResult {
    let url = format!("{}{}", endpoint, health_path);
    let start = Instant::now();

    let result = timeout(timeout_duration, client.get(&url).send()).await;

    let latency_ms = start.elapsed().as_millis() as u64;
    let timestamp = Utc::now();

    match result {
        Ok(Ok(response)) => {
            let status_code = response.status().as_u16();
            let success = response.status().is_success();
            HealthProbeResult {
                success,
                status_code: Some(status_code),
                latency_ms,
                timestamp,
                error: if success {
                    None
                } else {
                    Some(format!("HTTP {}", status_code))
                },
            }
        }
        Ok(Err(e)) => HealthProbeResult {
            success: false,
            status_code: None,
            latency_ms,
            timestamp,
            error: Some(e.to_string()),
        },
        Err(_) => HealthProbeResult {
            success: false,
            status_code: None,
            latency_ms: timeout_duration.as_millis() as u64,
            timestamp,
            error: Some("Timeout".to_string()),
        },
    }
}

/// Run health checks for all replicas in the cluster
///
/// This function:
/// 1. Collects all known replicas from node statuses
/// 2. Probes each replica's health endpoint
/// 3. Updates the health state in the controller
/// 4. Updates replica status based on consecutive failures/successes
pub async fn check_cluster_health(
    controller: &Arc<ClusterController>,
    client: &Client,
    config: &HealthCheckerConfig,
) {
    let nodes = controller.list_nodes();
    let timeout_duration = Duration::from_secs(config.timeout_secs);

    // Collect all replicas to probe
    let mut replicas_to_probe: Vec<(String, String, String, String, u16)> = Vec::new();

    for node in &nodes {
        let node_name = &node.metadata.name;
        let node_address = &node.spec.address;

        if let Some(status) = &node.status {
            for pipeline_info in &status.pipelines {
                replicas_to_probe.push((
                    node_name.clone(),
                    node_address.clone(),
                    pipeline_info.namespace.clone(),
                    pipeline_info.name.clone(),
                    pipeline_info.port,
                ));
            }
        }
    }

    if replicas_to_probe.is_empty() {
        trace!("No replicas to probe");
        return;
    }

    debug!("Probing {} replicas", replicas_to_probe.len());

    // Probe all replicas concurrently
    let probe_futures: Vec<_> = replicas_to_probe
        .iter()
        .map(|(node_name, node_address, namespace, pipeline_name, port)| {
            let endpoint = format!("http://{}:{}", node_address, port);
            let health_path = config.health_path.clone();
            let client = client.clone();

            async move {
                let result = probe_endpoint(&client, &endpoint, &health_path, timeout_duration).await;
                (node_name.clone(), node_address.clone(), namespace.clone(), pipeline_name.clone(), *port, result)
            }
        })
        .collect();

    let results = futures::future::join_all(probe_futures).await;

    // Update health states
    for (node_name, node_address, namespace, pipeline_name, port, probe_result) in results {
        let key = format!("{}:{}:{}:{}", node_name, namespace, pipeline_name, port);

        // Get or create health state
        let mut health_state = controller
            .get_replica_health(&key)
            .unwrap_or_else(|| ReplicaHealthState::new(&node_name, &node_address, &namespace, &pipeline_name, port));

        // Update counters
        if probe_result.success {
            health_state.consecutive_successes += 1;
            health_state.consecutive_failures = 0;

            // Mark as running if we hit success threshold
            if health_state.consecutive_successes >= config.success_threshold {
                if health_state.status != ReplicaStatus::Running {
                    health_state.status = ReplicaStatus::Running;
                    health_state.ready_since = Some(Utc::now());
                    debug!(
                        "Replica {} is now healthy ({}ms)",
                        key, probe_result.latency_ms
                    );
                }
            }
        } else {
            health_state.consecutive_failures += 1;
            health_state.consecutive_successes = 0;

            // Mark as unhealthy if we hit failure threshold
            if health_state.consecutive_failures >= config.failure_threshold {
                if health_state.status == ReplicaStatus::Running {
                    warn!(
                        "Replica {} is now unhealthy after {} failures: {:?}",
                        key, health_state.consecutive_failures, probe_result.error
                    );
                    health_state.status = ReplicaStatus::Unhealthy;
                    health_state.ready_since = None;
                }
            }
        }

        health_state.last_probe = Some(probe_result);

        // Store updated state
        controller.update_replica_health(key, health_state);
    }

    // Clean up stale health states (replicas that no longer exist)
    let active_keys: std::collections::HashSet<_> = replicas_to_probe
        .iter()
        .map(|(node, _, ns, name, port)| format!("{}:{}:{}:{}", node, ns, name, port))
        .collect();

    controller.cleanup_stale_health_states(&active_keys);
}

/// Get a summary of cluster health
pub fn get_cluster_health_summary(controller: &ClusterController) -> ClusterHealthSummary {
    let health_states = controller.list_replica_health();

    let mut total = 0;
    let mut healthy = 0;
    let mut unhealthy = 0;
    let mut starting = 0;
    let mut unknown = 0;

    for state in &health_states {
        total += 1;
        match state.status {
            ReplicaStatus::Running => healthy += 1,
            ReplicaStatus::Unhealthy => unhealthy += 1,
            ReplicaStatus::Starting => starting += 1,
            ReplicaStatus::Failed => unhealthy += 1,
            ReplicaStatus::Terminating => {}
        }
    }

    // If we have no health states yet, count from node pipelines
    if total == 0 {
        let nodes = controller.list_nodes();
        for node in nodes {
            if let Some(status) = node.status {
                for pipeline in status.pipelines {
                    total += 1;
                    match pipeline.status {
                        ReplicaStatus::Running => healthy += 1,
                        ReplicaStatus::Unhealthy | ReplicaStatus::Failed => unhealthy += 1,
                        ReplicaStatus::Starting => starting += 1,
                        ReplicaStatus::Terminating => {}
                    }
                }
            }
        }
    }

    // Calculate unknown (replicas we haven't probed yet)
    if total > 0 && healthy + unhealthy + starting == 0 {
        unknown = total;
    }

    ClusterHealthSummary {
        total_replicas: total,
        healthy_replicas: healthy,
        unhealthy_replicas: unhealthy,
        starting_replicas: starting,
        unknown_replicas: unknown,
        replicas: health_states,
    }
}

/// Summary of cluster health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealthSummary {
    pub total_replicas: u32,
    pub healthy_replicas: u32,
    pub unhealthy_replicas: u32,
    pub starting_replicas: u32,
    pub unknown_replicas: u32,
    pub replicas: Vec<ReplicaHealthState>,
}

impl ClusterHealthSummary {
    /// Overall cluster health status
    pub fn status(&self) -> &'static str {
        if self.total_replicas == 0 {
            "Empty"
        } else if self.unhealthy_replicas > 0 {
            "Degraded"
        } else if self.starting_replicas > 0 {
            "Starting"
        } else if self.unknown_replicas > 0 {
            "Unknown"
        } else if self.healthy_replicas == self.total_replicas {
            "Healthy"
        } else {
            "Degraded"
        }
    }

    /// Format as a professional table
    pub fn format_table(&self) -> String {
        use std::fmt::Write;

        let mut output = String::new();

        // Header
        writeln!(output, "\n╔══════════════════════════════════════════════════════════════════════════════════════════╗").unwrap();
        writeln!(output, "║                              CLUSTER HEALTH STATUS                                       ║").unwrap();
        writeln!(output, "╠══════════════════════════════════════════════════════════════════════════════════════════╣").unwrap();
        writeln!(
            output,
            "║  Status: {:<10}  │  Total: {:>3}  │  Healthy: {:>3}  │  Unhealthy: {:>3}  │  Starting: {:>3}  ║",
            self.status(),
            self.total_replicas,
            self.healthy_replicas,
            self.unhealthy_replicas,
            self.starting_replicas
        ).unwrap();
        writeln!(output, "╠══════════════════════════════════════════════════════════════════════════════════════════╣").unwrap();

        if self.replicas.is_empty() {
            writeln!(output, "║  No replicas deployed                                                                    ║").unwrap();
        } else {
            // Column headers
            writeln!(
                output,
                "║  {:<12} │ {:<20} │ {:<10} │ {:<8} │ {:<10} │ {:<8} │ {:<6} ║",
                "NODE", "PIPELINE", "STATUS", "UPTIME", "LAST CHECK", "LATENCY", "ERRORS"
            ).unwrap();
            writeln!(output, "╟──────────────┼──────────────────────┼────────────┼──────────┼────────────┼──────────┼────────╢").unwrap();

            for replica in &self.replicas {
                let status_str = match replica.status {
                    ReplicaStatus::Running => "✓ Running",
                    ReplicaStatus::Unhealthy => "✗ Unhealthy",
                    ReplicaStatus::Starting => "◐ Starting",
                    ReplicaStatus::Failed => "✗ Failed",
                    ReplicaStatus::Terminating => "◑ Stopping",
                };

                let last_check = replica.last_probe.as_ref().map(|p| {
                    let ago = Utc::now() - p.timestamp;
                    format!("{}s ago", ago.num_seconds())
                }).unwrap_or_else(|| "-".to_string());

                let latency = replica.last_probe.as_ref()
                    .map(|p| format!("{}ms", p.latency_ms))
                    .unwrap_or_else(|| "-".to_string());

                let pipeline_display = if replica.namespace == "default" {
                    replica.pipeline_name.clone()
                } else {
                    format!("{}/{}", replica.namespace, replica.pipeline_name)
                };

                writeln!(
                    output,
                    "║  {:<12} │ {:<20} │ {:<10} │ {:<8} │ {:<10} │ {:<8} │ {:>6} ║",
                    truncate(&replica.node_name, 12),
                    truncate(&pipeline_display, 20),
                    status_str,
                    replica.uptime_str(),
                    last_check,
                    latency,
                    replica.consecutive_failures
                ).unwrap();
            }
        }

        writeln!(output, "╚══════════════════════════════════════════════════════════════════════════════════════════╝").unwrap();

        output
    }
}

/// Truncate a string to a maximum length
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h1m");
        assert_eq!(format_duration(Duration::from_secs(90061)), "1d1h");
    }

    #[test]
    fn test_replica_health_state_key() {
        let state = ReplicaHealthState::new("dgx", "192.168.1.1", "default", "chatbot", 8080);
        assert_eq!(state.key, "dgx:default:chatbot:8080");
        assert_eq!(state.endpoint, "http://192.168.1.1:8080");
    }

    #[test]
    fn test_cluster_health_summary_status() {
        let summary = ClusterHealthSummary {
            total_replicas: 3,
            healthy_replicas: 3,
            unhealthy_replicas: 0,
            starting_replicas: 0,
            unknown_replicas: 0,
            replicas: vec![],
        };
        assert_eq!(summary.status(), "Healthy");

        let summary = ClusterHealthSummary {
            total_replicas: 3,
            healthy_replicas: 2,
            unhealthy_replicas: 1,
            starting_replicas: 0,
            unknown_replicas: 0,
            replicas: vec![],
        };
        assert_eq!(summary.status(), "Degraded");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello w…");
    }
}
