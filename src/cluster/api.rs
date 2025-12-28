//! Control Plane API Server
//!
//! Provides REST endpoints for managing the LLMNet cluster:
//! - Pipelines: deploy, list, get, delete, scale
//! - Nodes: register, list, heartbeat
//! - Namespaces: list
//! - Status: cluster health

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
        .route("/v1/pipelines", get(list_all_pipelines).post(deploy_pipeline))
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
    match state.controller.scale_pipeline(&namespace, &name, req.replicas) {
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
        Ok(_) => (
            StatusCode::CREATED,
            Json(NodeResponse::success(Some(node))),
        ),
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
            let score = node
                .status
                .as_ref()
                .and_then(|s| s.score.clone());
            match score {
                Some(s) => (StatusCode::OK, Json(NodeScoreResponse::with_score(name, s))).into_response(),
                None => (
                    StatusCode::OK,
                    Json(NodeScoreResponse::no_score(name)),
                )
                    .into_response(),
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
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
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
