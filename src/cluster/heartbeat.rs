//! Heartbeat client for worker nodes
//!
//! This module provides a background task that periodically sends heartbeats
//! to the control plane, including node metrics for scoring and scheduling.

use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use super::node::{NodeCapacity, NodeInfo, NodePhase, NodeStatus};
use super::HEARTBEAT_INTERVAL_SECS;
use crate::metrics::SharedMetricsCollector;

/// Configuration for the heartbeat client
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Control plane URL (e.g., "http://localhost:8181")
    pub control_plane_url: String,

    /// Name of this worker node
    pub node_name: String,

    /// Heartbeat interval in seconds (default: 30)
    pub interval_secs: u64,

    /// Node capacity for reporting
    pub capacity: NodeCapacity,

    /// Retry count before considering control plane unreachable
    pub max_retries: u32,
}

impl HeartbeatConfig {
    /// Create a new heartbeat config
    pub fn new(control_plane_url: impl Into<String>, node_name: impl Into<String>) -> Self {
        Self {
            control_plane_url: control_plane_url.into(),
            node_name: node_name.into(),
            interval_secs: HEARTBEAT_INTERVAL_SECS,
            capacity: NodeCapacity::default(),
            max_retries: 3,
        }
    }

    /// Set the heartbeat interval
    pub fn with_interval(mut self, secs: u64) -> Self {
        self.interval_secs = secs;
        self
    }

    /// Set the node capacity
    pub fn with_capacity(mut self, capacity: NodeCapacity) -> Self {
        self.capacity = capacity;
        self
    }
}

/// Heartbeat client that runs as a background task
pub struct HeartbeatClient {
    config: HeartbeatConfig,
    http_client: Client,
    metrics_collector: SharedMetricsCollector,
}

impl HeartbeatClient {
    /// Create a new heartbeat client
    pub fn new(config: HeartbeatConfig, metrics_collector: SharedMetricsCollector) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            http_client,
            metrics_collector,
        }
    }

    /// Run the heartbeat loop
    ///
    /// This should be spawned as a background task. It will run until the
    /// shutdown signal is received.
    pub async fn run(self, mut shutdown: watch::Receiver<bool>) {
        let interval = Duration::from_secs(self.config.interval_secs);
        let mut consecutive_failures = 0u32;

        info!(
            "Starting heartbeat client: node={}, control_plane={}, interval={}s",
            self.config.node_name, self.config.control_plane_url, self.config.interval_secs
        );

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    match self.send_heartbeat().await {
                        Ok(_) => {
                            if consecutive_failures > 0 {
                                info!("Heartbeat recovered after {} failures", consecutive_failures);
                            }
                            consecutive_failures = 0;
                            debug!("Heartbeat sent successfully");
                        }
                        Err(e) => {
                            consecutive_failures += 1;
                            if consecutive_failures >= self.config.max_retries {
                                error!(
                                    "Heartbeat failed {} consecutive times: {}",
                                    consecutive_failures, e
                                );
                            } else {
                                warn!("Heartbeat failed (attempt {}): {}", consecutive_failures, e);
                            }
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        info!("Heartbeat client shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Send a single heartbeat to the control plane
    async fn send_heartbeat(&self) -> Result<(), HeartbeatError> {
        // Collect metrics
        let metrics = {
            let mut collector = self.metrics_collector.write().await;
            collector.collect()
        };

        // Build node status
        let status = NodeStatus {
            phase: NodePhase::Ready,
            conditions: vec![],
            capacity: self.config.capacity.clone(),
            allocatable: self.config.capacity.clone(),
            pipelines: vec![], // TODO: Track running pipelines
            last_heartbeat: Utc::now(),
            node_info: NodeInfo::from_system(),
            metrics: Some(metrics),
            score: None, // Calculated by control plane
        };

        // Send heartbeat
        let url = format!(
            "{}/v1/nodes/{}/heartbeat",
            self.config.control_plane_url, self.config.node_name
        );

        let response = self
            .http_client
            .post(&url)
            .json(&status)
            .send()
            .await
            .map_err(HeartbeatError::RequestFailed)?;

        if !response.status().is_success() {
            let status_code = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(HeartbeatError::ServerError {
                status: status_code.as_u16(),
                message: body,
            });
        }

        Ok(())
    }
}

/// Errors that can occur during heartbeat
#[derive(Debug, thiserror::Error)]
pub enum HeartbeatError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Server error {status}: {message}")]
    ServerError { status: u16, message: String },
}

/// Spawn the heartbeat client as a background task
///
/// Returns a shutdown sender that can be used to stop the heartbeat loop.
pub fn spawn_heartbeat(
    config: HeartbeatConfig,
    metrics_collector: SharedMetricsCollector,
) -> watch::Sender<bool> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let client = HeartbeatClient::new(config, metrics_collector);

    tokio::spawn(async move {
        client.run(shutdown_rx).await;
    });

    shutdown_tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_config_builder() {
        let config = HeartbeatConfig::new("http://localhost:8181", "worker-1")
            .with_interval(60)
            .with_capacity(NodeCapacity::default());

        assert_eq!(config.control_plane_url, "http://localhost:8181");
        assert_eq!(config.node_name, "worker-1");
        assert_eq!(config.interval_secs, 60);
    }

    #[test]
    fn test_heartbeat_config_defaults() {
        let config = HeartbeatConfig::new("http://localhost:8181", "worker-1");

        assert_eq!(config.interval_secs, HEARTBEAT_INTERVAL_SECS);
        assert_eq!(config.max_retries, 3);
    }
}
