//! Control Plane API Server
//!
//! Provides REST endpoints for managing the LLMNet cluster:
//! - Pipelines: deploy, list, get, delete, scale
//! - Nodes: register, list, heartbeat
//! - Namespaces: list
//! - Status: cluster health

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
    Json, Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;

use super::{
    controller::ClusterController,
    node::{Node, NodeScore, NodeStatus},
    pipeline::{AutoscalingConfig, Pipeline},
    resources::{OperationStatus, ResourceList},
    ClusterStats,
};

/// Shared state for the control plane API
#[derive(Clone)]
pub struct ControlPlaneState {
    pub controller: Arc<ClusterController>,
}

impl ControlPlaneState {
    pub fn new() -> Self {
        Self {
            controller: Arc::new(ClusterController::new()),
        }
    }

    pub fn with_controller(controller: ClusterController) -> Self {
        Self {
            controller: Arc::new(controller),
        }
    }
}

impl Default for ControlPlaneState {
    fn default() -> Self {
        Self::new()
    }
}

/// Create the control plane router
pub fn create_control_plane_router(state: ControlPlaneState) -> Router {
    Router::new()
        // Cluster status
        .route("/v1/status", get(cluster_status))
        // Pipelines
        .route(
            "/v1/pipelines",
            get(list_all_pipelines).post(deploy_pipeline),
        )
        .route(
            "/v1/namespaces/{namespace}/pipelines",
            get(list_pipelines_in_namespace),
        )
        .route(
            "/v1/namespaces/{namespace}/pipelines/{name}",
            get(get_pipeline).delete(delete_pipeline),
        )
        .route(
            "/v1/namespaces/{namespace}/pipelines/{name}/scale",
            patch(scale_pipeline),
        )
        // Autoscaling
        .route(
            "/v1/namespaces/{namespace}/pipelines/{name}/autoscaling",
            get(get_autoscaling).put(update_autoscaling),
        )
        // Pipeline logs
        .route(
            "/v1/namespaces/{namespace}/pipelines/{name}/logs",
            get(stream_pipeline_logs),
        )
        // Nodes
        .route("/v1/nodes", get(list_nodes).post(register_node))
        .route("/v1/nodes/{name}", get(get_node).delete(unregister_node))
        .route("/v1/nodes/{name}/heartbeat", post(node_heartbeat))
        .route("/v1/nodes/{name}/score", get(get_node_score))
        .route("/v1/nodes/{name}/cordon", post(cordon_node))
        .route("/v1/nodes/{name}/uncordon", post(uncordon_node))
        // Namespaces
        .route("/v1/namespaces", get(list_namespaces))
        // Health check
        .route("/health", get(health_check))
        .with_state(state)
}

// ============================================================================
// Health & Status
// ============================================================================

async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}

async fn cluster_status(State(state): State<ControlPlaneState>) -> impl IntoResponse {
    let stats = state.controller.cluster_stats();
    Json(ClusterStatusResponse {
        status: "ok".to_string(),
        stats,
    })
}

#[derive(Serialize)]
struct ClusterStatusResponse {
    status: String,
    stats: ClusterStats,
}

// ============================================================================
// Pipeline Endpoints
// ============================================================================

async fn deploy_pipeline(
    State(state): State<ControlPlaneState>,
    Json(pipeline): Json<Pipeline>,
) -> impl IntoResponse {
    match state.controller.deploy_pipeline(pipeline) {
        Ok(deployed) => (StatusCode::CREATED, Json(DeployResponse::success(deployed))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(DeployResponse::error(e.to_string())),
        ),
    }
}

#[derive(Serialize)]
struct DeployResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pipeline: Option<Pipeline>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl DeployResponse {
    fn success(pipeline: Pipeline) -> Self {
        Self {
            success: true,
            pipeline: Some(pipeline),
            error: None,
        }
    }

    fn error(msg: String) -> Self {
        Self {
            success: false,
            pipeline: None,
            error: Some(msg),
        }
    }
}

async fn list_all_pipelines(State(state): State<ControlPlaneState>) -> impl IntoResponse {
    let pipelines = state.controller.list_all_pipelines();
    Json(ResourceList::new("PipelineList", pipelines))
}

async fn list_pipelines_in_namespace(
    State(state): State<ControlPlaneState>,
    Path(namespace): Path<String>,
) -> impl IntoResponse {
    let pipelines = state.controller.list_pipelines(&namespace);
    Json(ResourceList::new("PipelineList", pipelines))
}

async fn get_pipeline(
    State(state): State<ControlPlaneState>,
    Path((namespace, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.controller.get_pipeline(&namespace, &name) {
        Some(pipeline) => (StatusCode::OK, Json(Some(pipeline))).into_response(),
        None => (StatusCode::NOT_FOUND, Json::<Option<Pipeline>>(None)).into_response(),
    }
}

async fn delete_pipeline(
    State(state): State<ControlPlaneState>,
    Path((namespace, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.controller.delete_pipeline(&namespace, &name) {
        Ok(_) => (
            StatusCode::OK,
            Json(OperationStatus::success("Pipeline deleted")),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure(e.to_string())),
        ),
    }
}

#[derive(Deserialize)]
struct ScaleRequest {
    replicas: u32,
}

async fn scale_pipeline(
    State(state): State<ControlPlaneState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(req): Json<ScaleRequest>,
) -> impl IntoResponse {
    match state
        .controller
        .scale_pipeline(&namespace, &name, req.replicas)
    {
        Ok(pipeline) => (StatusCode::OK, Json(DeployResponse::success(pipeline))),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(DeployResponse::error(e.to_string())),
        ),
    }
}

// ============================================================================
// Autoscaling Endpoints
// ============================================================================

async fn get_autoscaling(
    State(state): State<ControlPlaneState>,
    Path((namespace, name)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.controller.get_pipeline(&namespace, &name) {
        Some(pipeline) => {
            let response = AutoscalingResponse {
                pipeline_name: pipeline.metadata.name.clone(),
                namespace: pipeline.metadata.namespace.clone(),
                current_replicas: pipeline.spec.replicas,
                autoscaling: pipeline.spec.autoscaling.clone(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure("Pipeline not found")),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
struct AutoscalingResponse {
    #[serde(rename = "pipelineName")]
    pipeline_name: String,
    namespace: String,
    #[serde(rename = "currentReplicas")]
    current_replicas: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    autoscaling: Option<AutoscalingConfig>,
}

async fn update_autoscaling(
    State(state): State<ControlPlaneState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(config): Json<AutoscalingConfig>,
) -> impl IntoResponse {
    match state.controller.get_pipeline(&namespace, &name) {
        Some(mut pipeline) => {
            pipeline.spec.autoscaling = Some(config);
            match state.controller.update_pipeline(pipeline) {
                Ok(updated) => (StatusCode::OK, Json(DeployResponse::success(updated))),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(DeployResponse::error(e.to_string())),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(DeployResponse::error("Pipeline not found".to_string())),
        ),
    }
}

// ============================================================================
// Node Endpoints
// ============================================================================

async fn list_nodes(State(state): State<ControlPlaneState>) -> impl IntoResponse {
    let nodes = state.controller.list_nodes();
    Json(ResourceList::new("NodeList", nodes))
}

async fn register_node(
    State(state): State<ControlPlaneState>,
    Json(node): Json<Node>,
) -> impl IntoResponse {
    match state.controller.register_node(node.clone()) {
        Ok(_) => (StatusCode::CREATED, Json(NodeResponse::success(Some(node)))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(NodeResponse::error(e.to_string())),
        ),
    }
}

#[derive(Serialize)]
struct NodeResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    node: Option<Node>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl NodeResponse {
    fn success(node: Option<Node>) -> Self {
        Self {
            success: true,
            node,
            error: None,
        }
    }

    fn error(msg: String) -> Self {
        Self {
            success: false,
            node: None,
            error: Some(msg),
        }
    }
}

async fn get_node(
    State(state): State<ControlPlaneState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.controller.get_node(&name) {
        Some(node) => (StatusCode::OK, Json(Some(node))).into_response(),
        None => (StatusCode::NOT_FOUND, Json::<Option<Node>>(None)).into_response(),
    }
}

async fn unregister_node(
    State(state): State<ControlPlaneState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.controller.unregister_node(&name) {
        Ok(_) => (
            StatusCode::OK,
            Json(OperationStatus::success("Node unregistered")),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure(e.to_string())),
        ),
    }
}

async fn node_heartbeat(
    State(state): State<ControlPlaneState>,
    Path(name): Path<String>,
    Json(status): Json<NodeStatus>,
) -> impl IntoResponse {
    match state.controller.update_node_status(&name, status) {
        Ok(_) => (
            StatusCode::OK,
            Json(OperationStatus::success("Heartbeat received")),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure(e.to_string())),
        ),
    }
}

async fn get_node_score(
    State(state): State<ControlPlaneState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.controller.get_node(&name) {
        Some(node) => {
            let score = node.status.as_ref().and_then(|s| s.score.clone());
            match score {
                Some(s) => {
                    (StatusCode::OK, Json(NodeScoreResponse::with_score(name, s))).into_response()
                }
                None => (StatusCode::OK, Json(NodeScoreResponse::no_score(name))).into_response(),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure("Node not found")),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
struct NodeScoreResponse {
    #[serde(rename = "nodeName")]
    node_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<NodeScore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl NodeScoreResponse {
    fn with_score(name: String, score: NodeScore) -> Self {
        Self {
            node_name: name,
            score: Some(score),
            message: None,
        }
    }

    fn no_score(name: String) -> Self {
        Self {
            node_name: name,
            score: None,
            message: Some("No metrics available yet".to_string()),
        }
    }
}

async fn cordon_node(
    State(state): State<ControlPlaneState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.controller.cordon_node(&name) {
        Ok(_) => (
            StatusCode::OK,
            Json(OperationStatus::success("Node cordoned")),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure(e.to_string())),
        ),
    }
}

async fn uncordon_node(
    State(state): State<ControlPlaneState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.controller.uncordon_node(&name) {
        Ok(_) => (
            StatusCode::OK,
            Json(OperationStatus::success("Node uncordoned")),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(OperationStatus::failure(e.to_string())),
        ),
    }
}

// ============================================================================
// Namespace Endpoints
// ============================================================================

async fn list_namespaces(State(state): State<ControlPlaneState>) -> impl IntoResponse {
    let namespaces = state.controller.list_namespaces();
    Json(ResourceList::new("NamespaceList", namespaces))
}

// ============================================================================
// Pipeline Logs
// ============================================================================

/// Query parameters for logs endpoint
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Follow log output (like tail -f)
    #[serde(default)]
    pub follow: bool,
    /// Number of lines to show from the end
    #[serde(default = "default_tail")]
    pub tail: usize,
    /// Container name (optional, defaults to first docker container in composition)
    pub container: Option<String>,
}

fn default_tail() -> usize {
    100
}

/// Stream logs for a pipeline's containers
async fn stream_pipeline_logs(
    State(state): State<ControlPlaneState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(params): Query<LogsQuery>,
) -> impl IntoResponse {
    // Get the pipeline
    let pipeline = match state.controller.get_pipeline(&namespace, &name) {
        Some(p) => p,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Body::from(format!("Pipeline '{}/{}' not found", namespace, name)),
            )
                .into_response();
        }
    };

    // Find the container name from the composition
    let container_name = if let Some(name) = params.container {
        name
    } else {
        // Find first model with docker config and a name
        let mut found: Option<String> = None;
        for model_def in pipeline.spec.composition.models.values() {
            let config = model_def.to_config();
            if let Some(docker) = &config.docker {
                if let Some(name) = &docker.name {
                    found = Some(name.clone());
                    break;
                }
            }
        }
        match found {
            Some(n) => n,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Body::from(
                        "No docker container name found in composition. Specify ?container=NAME",
                    ),
                )
                    .into_response();
            }
        }
    };

    // Find the worker node that has this pipeline
    // First try pipeline tracking, then fallback to checking which worker has the container
    let nodes = state.controller.list_nodes();

    // Method 1: Check node pipeline tracking
    let found_node = nodes.iter().find(|n| {
        n.status
            .as_ref()
            .map(|s| {
                s.pipelines
                    .iter()
                    .any(|p| p.namespace == namespace && p.name == name)
            })
            .unwrap_or(false)
    });

    let worker_url = if let Some(node) = found_node {
        format!("http://{}:{}", node.spec.address, node.spec.port)
    } else {
        // Method 2: Query each node to see if it has the container
        // This is a fallback for when pipeline tracking isn't working
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();

        let mut found_worker: Option<String> = None;
        for node in &nodes {
            let url = format!(
                "http://{}:{}/v1/containers",
                node.spec.address, node.spec.port
            );
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(containers) = body["containers"].as_array() {
                        if containers
                            .iter()
                            .any(|c| c.as_str() == Some(&container_name))
                        {
                            found_worker =
                                Some(format!("http://{}:{}", node.spec.address, node.spec.port));
                            break;
                        }
                    }
                }
            }
        }

        match found_worker {
            Some(url) => url,
            None => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Body::from("Pipeline not scheduled to any worker or container not found"),
                )
                    .into_response();
            }
        }
    };

    // Proxy request to worker
    let client = reqwest::Client::new();
    let url = format!(
        "{}/v1/containers/{}/logs?follow={}&tail={}",
        worker_url, container_name, params.follow, params.tail
    );

    match client.get(&url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return (
                    StatusCode::from_u16(status.as_u16())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Body::from(body),
                )
                    .into_response();
            }

            // Stream the response body
            let stream = response
                .bytes_stream()
                .map(|result| result.map_err(std::io::Error::other));
            let body = Body::from_stream(stream);
            (StatusCode::OK, body).into_response()
        }
        Err(e) => {
            warn!("Failed to proxy logs request to {}: {}", worker_url, e);
            (
                StatusCode::BAD_GATEWAY,
                Body::from(format!("Failed to connect to worker: {}", e)),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn create_test_app() -> Router {
        let state = ControlPlaneState::new();
        create_control_plane_router(state)
    }

    #[tokio::test]
    async fn test_health_check() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_cluster_status() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_pipelines_empty() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/pipelines")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_nodes_empty() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/nodes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_list_namespaces() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/namespaces")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_deploy_pipeline() {
        let app = create_test_app();

        let pipeline_json = r#"{
            "apiVersion": "llmnet/v1",
            "kind": "Pipeline",
            "metadata": {
                "name": "test-pipeline",
                "namespace": "default"
            },
            "spec": {
                "replicas": 1,
                "composition": {
                    "models": {},
                    "architecture": [
                        {"name": "router", "layer": 0, "adapter": "openai-api"},
                        {"name": "output", "adapter": "output"}
                    ]
                }
            }
        }"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/pipelines")
                    .header("content-type", "application/json")
                    .body(Body::from(pipeline_json))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_get_pipeline_not_found() {
        let app = create_test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/namespaces/default/pipelines/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_register_node() {
        let app = create_test_app();

        let node_json = r#"{
            "apiVersion": "llmnet/v1",
            "kind": "Node",
            "metadata": {
                "name": "worker-1"
            },
            "spec": {
                "address": "192.168.1.100",
                "port": 8080
            }
        }"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes")
                    .header("content-type", "application/json")
                    .body(Body::from(node_json))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }
}
