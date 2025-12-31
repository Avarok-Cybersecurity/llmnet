//! Cluster Orchestrator - watches pipelines and schedules them to workers
//!
//! The orchestrator runs as a background task on the control plane and:
//! - Watches for pipelines in "Pending" state
//! - Schedules them to available workers using the scheduler
//! - Sends pipeline assignments to workers via HTTP
//! - Updates pipeline status based on worker feedback

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use super::controller::ClusterController;
use super::node::ReplicaStatus;
use super::pipeline::{PipelineCondition, PipelineStatus};
use crate::config::Composition;

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// How often to check for pending pipelines (seconds)
    pub reconcile_interval_secs: u64,
    /// Timeout for worker HTTP requests (seconds)
    pub worker_request_timeout_secs: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            reconcile_interval_secs: 5,
            worker_request_timeout_secs: 30,
        }
    }
}

/// Assignment sent to a worker node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineAssignment {
    /// Pipeline namespace
    pub namespace: String,
    /// Pipeline name
    pub name: String,
    /// The composition to run
    pub composition: Composition,
    /// Port to serve the pipeline on
    pub port: u16,
    /// Number of replicas this worker should run
    pub replicas: u32,
}

/// Response from worker after receiving assignment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Spawn the orchestrator as a background task
pub fn spawn_orchestrator(
    controller: Arc<ClusterController>,
    config: OrchestratorConfig,
) -> watch::Sender<()> {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(());

    tokio::spawn(async move {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.worker_request_timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        let mut ticker = interval(Duration::from_secs(config.reconcile_interval_secs));

        info!(
            "Orchestrator started, reconciling every {}s",
            config.reconcile_interval_secs
        );

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    reconcile_pipelines(&controller, &client).await;
                    reconcile_health(&controller);
                }
                _ = shutdown_rx.changed() => {
                    info!("Orchestrator shutting down");
                    break;
                }
            }
        }
    });

    shutdown_tx
}

/// Reconcile all pipelines - the main orchestration loop
async fn reconcile_pipelines(controller: &ClusterController, client: &Client) {
    let pipelines = controller.list_all_pipelines();

    for pipeline in pipelines {
        let status = pipeline.status.as_ref();

        // Check if pipeline needs scheduling (no replicas running yet)
        let needs_scheduling = status
            .map(|s| s.replicas == 0 && s.ready_replicas == 0)
            .unwrap_or(true);

        if !needs_scheduling {
            continue;
        }

        debug!(
            "Pipeline {}/{} needs scheduling",
            pipeline.metadata.namespace, pipeline.metadata.name
        );

        // Try to schedule the pipeline
        match schedule_pipeline(controller, client, &pipeline).await {
            Ok(endpoints) => {
                // Update pipeline status
                let mut new_status = status.cloned().unwrap_or_else(PipelineStatus::initial);
                new_status.replicas = pipeline.spec.replicas;
                new_status.endpoints = endpoints;
                new_status.conditions.push(PipelineCondition::new(
                    "Scheduled",
                    "True",
                    "ReplicasScheduled",
                    format!("{} replica(s) scheduled to workers", pipeline.spec.replicas),
                ));

                if let Err(e) = controller.update_pipeline_status(
                    &pipeline.metadata.namespace,
                    &pipeline.metadata.name,
                    new_status,
                ) {
                    error!("Failed to update pipeline status: {}", e);
                }

                info!(
                    "Pipeline {}/{} scheduled successfully",
                    pipeline.metadata.namespace, pipeline.metadata.name
                );
            }
            Err(e) => {
                warn!(
                    "Failed to schedule pipeline {}/{}: {}",
                    pipeline.metadata.namespace, pipeline.metadata.name, e
                );

                // Update status with failure condition
                let mut new_status = status.cloned().unwrap_or_else(PipelineStatus::initial);
                new_status.conditions.push(PipelineCondition::new(
                    "Scheduled",
                    "False",
                    "SchedulingFailed",
                    e.to_string(),
                ));

                let _ = controller.update_pipeline_status(
                    &pipeline.metadata.namespace,
                    &pipeline.metadata.name,
                    new_status,
                );
            }
        }
    }
}

/// Schedule a single pipeline to workers
async fn schedule_pipeline(
    controller: &ClusterController,
    client: &Client,
    pipeline: &super::Pipeline,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get scheduling decisions
    let schedule = controller.schedule_replicas(pipeline)?;

    if schedule.is_empty() {
        return Err("No nodes available for scheduling".into());
    }

    let mut endpoints = Vec::new();

    // Send assignment to each worker
    for (node_name, replica_count) in schedule {
        let node = controller
            .get_node(&node_name)
            .ok_or_else(|| format!("Node {} not found", node_name))?;

        let worker_url = format!(
            "http://{}:{}/v1/assignments",
            node.spec.address, node.spec.port
        );

        let assignment = PipelineAssignment {
            namespace: pipeline.metadata.namespace.clone(),
            name: pipeline.metadata.name.clone(),
            composition: pipeline.spec.composition.clone(),
            port: pipeline.spec.port,
            replicas: replica_count,
        };

        debug!(
            "Sending assignment to worker {} at {}",
            node_name, worker_url
        );

        match client.post(&worker_url).json(&assignment).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<AssignmentResponse>().await {
                    Ok(ar) if ar.success => {
                        if let Some(endpoint) = ar.endpoint {
                            endpoints.push(endpoint);
                        }
                        // Track pipeline on this node
                        if let Err(e) = controller.add_pipeline_to_node(
                            &node_name,
                            &pipeline.metadata.namespace,
                            &pipeline.metadata.name,
                            pipeline.spec.port,
                        ) {
                            warn!("Failed to track pipeline on node {}: {}", node_name, e);
                        }
                        info!(
                            "Worker {} accepted assignment for {}/{}",
                            node_name, pipeline.metadata.namespace, pipeline.metadata.name
                        );
                    }
                    Ok(ar) => {
                        warn!(
                            "Worker {} rejected assignment: {}",
                            node_name,
                            ar.error.unwrap_or_else(|| "unknown error".to_string())
                        );
                    }
                    Err(e) => {
                        warn!("Failed to parse worker response from {}: {}", node_name, e);
                    }
                }
            }
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!("Worker {} returned error {}: {}", node_name, status, body);
            }
            Err(e) => {
                warn!("Failed to contact worker {}: {}", node_name, e);
            }
        }
    }

    if endpoints.is_empty() {
        Err("No workers successfully accepted the assignment".into())
    } else {
        Ok(endpoints)
    }
}

/// Reconcile pipeline health status based on node heartbeats
///
/// This function updates pipeline ready_replicas and available_replicas
/// by counting replicas reported in node status.pipelines
fn reconcile_health(controller: &ClusterController) {
    let pipelines = controller.list_all_pipelines();
    let nodes = controller.list_nodes();

    for pipeline in pipelines {
        let namespace = &pipeline.metadata.namespace;
        let name = &pipeline.metadata.name;

        // Count replicas across all nodes
        let mut ready = 0u32;
        let mut available = 0u32;

        for node in &nodes {
            if let Some(status) = &node.status {
                for np in &status.pipelines {
                    if &np.namespace == namespace && &np.name == name {
                        ready += 1;
                        if np.status == ReplicaStatus::Running {
                            available += 1;
                        }
                    }
                }
            }
        }

        // Only update if status differs from current
        let current_status = pipeline.status.as_ref();
        let needs_update = current_status
            .map(|s| s.ready_replicas != ready || s.available_replicas != available)
            .unwrap_or(true);

        if needs_update {
            let mut new_status = current_status
                .cloned()
                .unwrap_or_else(PipelineStatus::initial);
            new_status.ready_replicas = ready;
            new_status.available_replicas = available;
            new_status.unavailable_replicas = pipeline.spec.replicas.saturating_sub(available);

            // Add/update Available condition
            if available >= pipeline.spec.replicas {
                new_status
                    .conditions
                    .retain(|c| c.condition_type != "Available");
                new_status.conditions.push(PipelineCondition::new(
                    "Available",
                    "True",
                    "MinimumReplicasAvailable",
                    format!(
                        "{}/{} replicas available",
                        available, pipeline.spec.replicas
                    ),
                ));
            }

            if let Err(e) = controller.update_pipeline_status(namespace, name, new_status) {
                warn!(
                    "Failed to update pipeline {}/{} health status: {}",
                    namespace, name, e
                );
            } else {
                debug!(
                    "Pipeline {}/{} health: ready={}, available={}",
                    namespace, name, ready, available
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.reconcile_interval_secs, 5);
        assert_eq!(config.worker_request_timeout_secs, 30);
    }

    #[test]
    fn test_pipeline_assignment_serialization() {
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "output", "adapter": "output"}
            ]
        }"#;
        let composition = crate::config::Composition::from_str(json).unwrap();

        let assignment = PipelineAssignment {
            namespace: "default".to_string(),
            name: "test-pipeline".to_string(),
            composition,
            port: 8080,
            replicas: 1,
        };

        let serialized = serde_json::to_string(&assignment).unwrap();
        assert!(serialized.contains("test-pipeline"));
        assert!(serialized.contains("default"));
    }
}
