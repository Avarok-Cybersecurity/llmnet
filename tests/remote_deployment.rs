//! Integration tests for remote deployment simulation
//!
//! These tests simulate remote deployment by running multiple instances
//! on localhost with different ports.

use std::net::TcpListener;
use std::time::Duration;

use llmnet::config::models::ModelConfig;
use llmnet::config::Composition;
use llmnet::runtime::new_shared_manager;
use llmnet::server::{create_router, AppState};

use axum::http::StatusCode;
use tokio::time::sleep;

/// Find an available port for testing
fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to address")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

/// Create a minimal composition for testing
fn minimal_composition() -> Composition {
    let json = r#"{
        "models": {},
        "architecture": [
            {"name": "router", "layer": 0, "adapter": "openai-api"},
            {"name": "output", "adapter": "output"}
        ]
    }"#;
    Composition::from_str(json).unwrap()
}

#[tokio::test]
async fn test_worker_runner_endpoints() {
    // Setup: Create worker with runner manager
    let worker_port = find_available_port();
    let composition = minimal_composition();
    let runner_manager = new_shared_manager();

    let state = AppState::new(composition).with_runner_manager(runner_manager);
    let app = create_router(state);

    // Spawn worker server
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", worker_port))
        .await
        .expect("Failed to bind worker");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{}", worker_port);

    // Test 1: List runners (should be empty)
    let response = client
        .get(format!("{}/v1/runners", base_url))
        .send()
        .await
        .expect("Failed to list runners");

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["runners"].as_array().unwrap().is_empty());

    // Test 2: Spawn a runner (external type - won't actually spawn)
    // Using external to test the endpoint without requiring ollama installed
    let config = ModelConfig::external("http://localhost:11434/v1");
    let spawn_request = serde_json::json!({
        "name": "test-model",
        "config": config
    });

    let response = client
        .post(format!("{}/v1/runners/spawn", base_url))
        .json(&spawn_request)
        .send()
        .await
        .expect("Failed to spawn runner");

    // External runners return an error since they're not spawnable
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("not spawned locally"));

    // Test 3: Health check still works
    let response = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("Failed health check");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_worker_without_runner_manager() {
    // Setup: Create worker without runner manager
    let worker_port = find_available_port();
    let composition = minimal_composition();
    let state = AppState::new(composition); // No runner manager
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", worker_port))
        .await
        .expect("Failed to bind worker");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{}", worker_port);

    // List runners should return empty (graceful handling)
    let response = client
        .get(format!("{}/v1/runners", base_url))
        .send()
        .await
        .expect("Failed to list runners");

    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(body["runners"].as_array().unwrap().is_empty());

    // Spawn should return SERVICE_UNAVAILABLE
    let config = ModelConfig::external("http://localhost:11434/v1");
    let spawn_request = serde_json::json!({
        "name": "test-model",
        "config": config
    });

    let response = client
        .post(format!("{}/v1/runners/spawn", base_url))
        .json(&spawn_request)
        .send()
        .await
        .expect("Failed to spawn runner");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_control_plane_dispatches_to_worker() {
    // This test simulates the control plane sending a spawn command to a worker
    // representing a "remote cluster"

    // Start worker on a port simulating a remote cluster
    let worker_port = find_available_port();
    let composition = minimal_composition();
    let runner_manager = new_shared_manager();
    let worker_state = AppState::new(composition.clone()).with_runner_manager(runner_manager);
    let worker_app = create_router(worker_state);

    let worker_listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", worker_port))
        .await
        .expect("Failed to bind worker");

    tokio::spawn(async move {
        axum::serve(worker_listener, worker_app).await.unwrap();
    });

    // Start control plane on another port
    let control_port = find_available_port();
    let control_state = AppState::new(composition);
    let control_app = create_router(control_state);

    let control_listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", control_port))
        .await
        .expect("Failed to bind control plane");

    tokio::spawn(async move {
        axum::serve(control_listener, control_app).await.unwrap();
    });

    sleep(Duration::from_millis(100)).await;

    let client = reqwest::Client::new();

    // Simulate control plane dispatching to worker
    // In a real scenario, the control plane would look up the worker URL
    // based on the architecture node's context setting
    let worker_url = format!("http://127.0.0.1:{}", worker_port);

    // Control plane sends spawn request to worker (simulating context dispatch)
    let config = ModelConfig::external("http://localhost:11434/v1");
    let spawn_request = serde_json::json!({
        "name": "remote-model",
        "config": config
    });

    let response = client
        .post(format!("{}/v1/runners/spawn", worker_url))
        .json(&spawn_request)
        .send()
        .await
        .expect("Failed to dispatch to worker");

    // External type returns error (expected - just testing the dispatch path)
    assert!(
        response.status() == StatusCode::INTERNAL_SERVER_ERROR
            || response.status() == StatusCode::OK
    );

    // Verify both services are running independently
    let control_health = client
        .get(format!("http://127.0.0.1:{}/health", control_port))
        .send()
        .await
        .expect("Failed control health check");
    assert_eq!(control_health.status(), StatusCode::OK);

    let worker_health = client
        .get(format!("{}/health", worker_url))
        .send()
        .await
        .expect("Failed worker health check");
    assert_eq!(worker_health.status(), StatusCode::OK);
}
