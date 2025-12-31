use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;
use tracing::error;
use uuid::Uuid;

use crate::client::Message;
use crate::cluster::{AssignmentResponse, PipelineAssignment};
use crate::config::models::{ModelConfig, RunnerType};
use crate::server::state::AppState;

/// OpenAI-compatible chat completion request
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stream: bool,
}

/// OpenAI-compatible chat completion response
#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ResponseChoice>,
    pub usage: ResponseUsage,
}

#[derive(Debug, Serialize)]
pub struct ResponseChoice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ResponseUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Health check endpoint
pub async fn health() -> impl IntoResponse {
    StatusCode::OK
}

/// Pipeline status endpoint
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let status = PipelineStatus {
        nodes: state.nodes.len(),
        active_requests: state.active_request_count(),
    };
    Json(status)
}

#[derive(Serialize)]
struct PipelineStatus {
    nodes: usize,
    active_requests: usize,
}

// ============================================================================
// Runner Management Endpoints (Worker Mode)
// ============================================================================

/// Request to spawn a runner
#[derive(Debug, Deserialize)]
pub struct SpawnRunnerRequest {
    pub name: String,
    pub config: ModelConfig,
}

/// Response from spawning a runner
#[derive(Debug, Serialize)]
pub struct SpawnRunnerResponse {
    pub name: String,
    pub endpoint: String,
    pub status: String,
}

/// List of running models
#[derive(Debug, Serialize)]
pub struct RunnerListResponse {
    pub runners: Vec<RunnerInfo>,
}

#[derive(Debug, Serialize)]
pub struct RunnerInfo {
    pub name: String,
    pub endpoint: Option<String>,
}

/// Spawn a model runner (worker endpoint)
pub async fn spawn_runner(
    State(state): State<AppState>,
    Json(request): Json<SpawnRunnerRequest>,
) -> impl IntoResponse {
    let Some(manager) = &state.runner_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Runner manager not available"
            })),
        );
    };

    match manager.spawn_runner(&request.name, &request.config).await {
        Ok(endpoint) => (
            StatusCode::OK,
            Json(serde_json::json!(SpawnRunnerResponse {
                name: request.name,
                endpoint,
                status: "running".to_string(),
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to spawn runner: {}", e)
            })),
        ),
    }
}

/// List running models (worker endpoint)
pub async fn list_runners(State(state): State<AppState>) -> impl IntoResponse {
    let Some(manager) = &state.runner_manager else {
        return Json(RunnerListResponse { runners: vec![] });
    };

    let runners = manager
        .list_running()
        .into_iter()
        .map(|name| {
            let endpoint = manager.get_endpoint(&name);
            RunnerInfo { name, endpoint }
        })
        .collect();

    Json(RunnerListResponse { runners })
}

/// Stop a running model (worker endpoint)
pub async fn stop_runner(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let Some(manager) = &state.runner_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Runner manager not available"
            })),
        );
    };

    match manager.stop_runner(&name).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "stopped",
                "name": name
            })),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("Failed to stop runner: {}", e)
            })),
        ),
    }
}

// ============================================================================
// Pipeline Assignment Endpoint (Worker Mode - receives work from Control Plane)
// ============================================================================

/// Receive a pipeline assignment from the control plane
///
/// This endpoint is called by the control plane orchestrator when scheduling
/// a pipeline to this worker node. It will:
/// 1. Spawn any required model runners (Docker, Ollama, etc.)
/// 2. Initialize the pipeline processor
/// 3. Return the endpoint where the pipeline is accessible
pub async fn receive_assignment(
    State(state): State<AppState>,
    Json(assignment): Json<PipelineAssignment>,
) -> impl IntoResponse {
    tracing::info!(
        "Received pipeline assignment: {}/{} with {} replicas",
        assignment.namespace,
        assignment.name,
        assignment.replicas
    );

    // Get the runner manager
    let Some(manager) = &state.runner_manager else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AssignmentResponse {
                success: false,
                endpoint: None,
                error: Some("Runner manager not available on this worker".to_string()),
            }),
        );
    };

    // Spawn runners for each model that needs one
    for (model_name, model_def) in &assignment.composition.models {
        let config = model_def.to_config();

        // Check if this model needs a runner (Docker, Ollama, vLLM, llama.cpp)
        let needs_runner = matches!(
            config.runner,
            RunnerType::Docker | RunnerType::Ollama | RunnerType::Vllm | RunnerType::LlamaCpp
        );

        if needs_runner {
            tracing::info!("Spawning {} runner for model '{}'...", config.type_name(), model_name);

            match manager.spawn_runner(model_name, &config).await {
                Ok(endpoint) => {
                    tracing::info!("Runner for '{}' ready at {}", model_name, endpoint);
                }
                Err(e) => {
                    tracing::error!("Failed to spawn runner for '{}': {}", model_name, e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(AssignmentResponse {
                            success: false,
                            endpoint: None,
                            error: Some(format!("Failed to spawn runner for '{}': {}", model_name, e)),
                        }),
                    );
                }
            }
        }
    }

    // Build the endpoint URL for this pipeline
    // In host networking mode, we use the assigned port
    let endpoint = format!("http://{}:{}", state.bind_addr, assignment.port);

    tracing::info!(
        "Pipeline {}/{} ready at {}",
        assignment.namespace,
        assignment.name,
        endpoint
    );

    (
        StatusCode::OK,
        Json(AssignmentResponse {
            success: true,
            endpoint: Some(endpoint),
            error: None,
        }),
    )
}

/// Chat completions endpoint (OpenAI-compatible)
pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    // Extract request ID from headers or generate new one
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or_else(Uuid::new_v4);

    // Extract the user prompt from messages
    let user_prompt = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Process through the pipeline if processor is available
    let content = if let Some(processor) = &state.processor {
        match processor.process(&user_prompt).await {
            Ok(response) => response,
            Err(e) => format!("Pipeline error: {}", e),
        }
    } else {
        format!("No pipeline processor configured for: {}", user_prompt)
    };

    let response = ChatCompletionResponse {
        id: format!("chatcmpl-{}", request_id),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: request.model.clone(),
        choices: vec![ResponseChoice {
            index: 0,
            message: Message {
                role: "assistant".to_string(),
                content,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: ResponseUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    };

    // Add request ID to response headers
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "x-request-id",
        request_id.to_string().parse().unwrap(),
    );

    (response_headers, Json(response))
}

/// Query parameters for logs endpoint
#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    /// Follow log output (like tail -f)
    #[serde(default)]
    pub follow: bool,
    /// Number of lines to show from the end
    #[serde(default = "default_tail")]
    pub tail: usize,
}

fn default_tail() -> usize {
    100
}

/// Stream container logs
pub async fn stream_logs(
    State(state): State<AppState>,
    Path(container): Path<String>,
    Query(params): Query<LogsQuery>,
) -> impl IntoResponse {
    let manager = match &state.runner_manager {
        Some(m) => m,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Body::from("Runner manager not available"),
            )
                .into_response();
        }
    };

    // Check if container exists in our tracked processes
    let containers = manager.list_containers();
    if !containers.contains(&container) {
        return (
            StatusCode::NOT_FOUND,
            Body::from(format!("Container '{}' not found", container)),
        )
            .into_response();
    }

    // Start streaming logs
    match manager
        .stream_container_logs(&container, params.follow, Some(params.tail))
        .await
    {
        Ok(mut child) => {
            // Merge stdout and stderr into a single stream
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();

            match (stdout, stderr) {
                (Some(stdout), Some(stderr)) => {
                    // Create streams from both
                    let stdout_stream = ReaderStream::new(stdout);
                    let stderr_stream = ReaderStream::new(stderr);

                    // Merge the streams
                    let merged = futures::stream::select(stdout_stream, stderr_stream);

                    let body = Body::from_stream(merged);
                    (StatusCode::OK, body).into_response()
                }
                (Some(stdout), None) => {
                    let stream = ReaderStream::new(stdout);
                    let body = Body::from_stream(stream);
                    (StatusCode::OK, body).into_response()
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Body::from("Failed to capture log output"),
                )
                    .into_response(),
            }
        }
        Err(e) => {
            error!("Failed to stream logs for '{}': {}", container, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::from(format!("Failed to stream logs: {}", e)),
            )
                .into_response()
        }
    }
}

/// List available containers
pub async fn list_containers(State(state): State<AppState>) -> impl IntoResponse {
    let containers = state
        .runner_manager
        .as_ref()
        .map(|m| m.list_containers())
        .unwrap_or_default();

    Json(serde_json::json!({
        "containers": containers
    }))
}

/// Create the Axum router
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/v1/chat/completions", post(chat_completions))
        // Runner management endpoints (worker mode)
        .route("/v1/runners", get(list_runners))
        .route("/v1/runners/spawn", post(spawn_runner))
        .route("/v1/runners/{name}", delete(stop_runner))
        // Pipeline assignment endpoint (control plane -> worker)
        .route("/v1/assignments", post(receive_assignment))
        // Container logs endpoints
        .route("/v1/containers", get(list_containers))
        .route("/v1/containers/{container}/logs", get(stream_logs))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Composition;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn create_test_app() -> Router {
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "output", "adapter": "output"}
            ]
        }"#;
        let comp = Composition::from_str(json).unwrap();
        let state = AppState::new(comp);
        create_router(state)
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_test_app();

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_status_endpoint() {
        let app = create_test_app();

        let response = app
            .oneshot(Request::builder().uri("/status").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_chat_completions_endpoint() {
        let app = create_test_app();

        let request_body = serde_json::json!({
            "model": "test-model",
            "messages": [
                {"role": "user", "content": "Hello"}
            ]
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
