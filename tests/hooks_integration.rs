//! Integration tests for hooks with thin HTTP servers
//!
//! These tests verify hook execution by creating thin HTTP servers that act as
//! function endpoints for pre and post hooks.

use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio::time::sleep;

use llmnet::config::{Composition, FunctionExecutor, FunctionType, SecretsManager};
use llmnet::runtime::hooks::{HookContext, HookExecutor};

/// Find an available port for testing
fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind to address")
        .local_addr()
        .expect("Failed to get local address")
        .port()
}

/// State for tracking hook invocations
#[derive(Debug, Clone)]
struct HookServerState {
    call_count: Arc<AtomicUsize>,
    transform_value: Option<String>,
}

/// Handler for pre-hook: logs and optionally transforms input
async fn pre_hook_handler(
    State(state): State<HookServerState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    state.call_count.fetch_add(1, Ordering::SeqCst);

    // If transform_value is set, return it; otherwise echo input
    if let Some(transform) = &state.transform_value {
        // Return just the transformed value
        Json(Value::String(transform.clone()))
    } else {
        // Echo the input for observe mode
        Json(payload.get("input").cloned().unwrap_or(Value::Null))
    }
}

/// Handler for post-hook: validates and optionally transforms output
async fn post_hook_handler(
    State(state): State<HookServerState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    state.call_count.fetch_add(1, Ordering::SeqCst);

    let output = payload.get("output").and_then(|v| v.as_str()).unwrap_or("");

    // If transform_value is set, append it to output
    if let Some(transform) = &state.transform_value {
        // Return just the transformed value
        Json(Value::String(format!("{} | {}", output, transform)))
    } else {
        Json(Value::String(output.to_string()))
    }
}

/// Handler that always fails with HTTP 500
async fn failing_hook_handler(
    State(state): State<HookServerState>,
) -> (axum::http::StatusCode, Json<Value>) {
    state.call_count.fetch_add(1, Ordering::SeqCst);
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Intentional failure for testing"})),
    )
}

/// Start a thin server for pre-hooks
async fn start_pre_hook_server(port: u16, transform_value: Option<String>) -> Arc<AtomicUsize> {
    let call_count = Arc::new(AtomicUsize::new(0));
    let state = HookServerState {
        call_count: call_count.clone(),
        transform_value,
    };

    let app = Router::new()
        .route("/hook", post(pre_hook_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to bind pre-hook server");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;
    call_count
}

/// Start a thin server for post-hooks
async fn start_post_hook_server(port: u16, transform_value: Option<String>) -> Arc<AtomicUsize> {
    let call_count = Arc::new(AtomicUsize::new(0));
    let state = HookServerState {
        call_count: call_count.clone(),
        transform_value,
    };

    let app = Router::new()
        .route("/hook", post(post_hook_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to bind post-hook server");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;
    call_count
}

/// Start a failing hook server
async fn start_failing_hook_server(port: u16) -> Arc<AtomicUsize> {
    let call_count = Arc::new(AtomicUsize::new(0));
    let state = HookServerState {
        call_count: call_count.clone(),
        transform_value: None,
    };

    let app = Router::new()
        .route("/hook", post(failing_hook_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .expect("Failed to bind failing-hook server");

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;
    call_count
}

// ============================================================================
// Pre-hook Tests
// ============================================================================

#[tokio::test]
async fn test_pre_hook_observe_mode() {
    // Start a thin server for pre-hook
    let pre_port = find_available_port();
    let call_count = start_pre_hook_server(pre_port, None).await;

    // Create function definition pointing to thin server
    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "log-input".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", pre_port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"input": "$INPUT", "node": "$NODE"})),
            timeout: 5,
        },
    );

    // Create hook executor
    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    // Create composition with pre-hook in observe mode
    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "pre": [
                        {{"function": "log-input", "mode": "observe"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "log-input": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"input": "$INPUT", "node": "$NODE"}}
            }}
        }}
    }}"#,
        pre_port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    // Execute pre-hooks
    let context = HookContext::new("test-node", "req-001");
    let input = Value::String("Hello, world!".to_string());

    let result = hook_executor
        .execute_pre_hooks(&node.hooks, input.clone(), &context)
        .await
        .unwrap();

    // Give observe mode hook time to complete (it's fire-and-forget)
    sleep(Duration::from_millis(100)).await;

    // In observe mode, input should be unchanged
    assert_eq!(result, input);

    // Hook should have been called
    assert!(call_count.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_pre_hook_transform_mode() {
    // Start a thin server that transforms input
    let pre_port = find_available_port();
    let call_count = start_pre_hook_server(pre_port, Some("TRANSFORMED INPUT".to_string())).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "transform-input".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", pre_port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"input": "$INPUT"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "pre": [
                        {{"function": "transform-input", "mode": "transform"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "transform-input": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"input": "$INPUT"}}
            }}
        }}
    }}"#,
        pre_port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    let context = HookContext::new("test-node", "req-002");
    let input = Value::String("Original input".to_string());

    let result = hook_executor
        .execute_pre_hooks(&node.hooks, input, &context)
        .await
        .unwrap();

    // Transform mode should change the input
    assert_eq!(result, Value::String("TRANSFORMED INPUT".to_string()));
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Post-hook Tests
// ============================================================================

#[tokio::test]
async fn test_post_hook_observe_mode() {
    let post_port = find_available_port();
    let call_count = start_post_hook_server(post_port, None).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "log-output".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", post_port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"output": "$OUTPUT", "node": "$NODE"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "post": [
                        {{"function": "log-output", "mode": "observe"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "log-output": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"output": "$OUTPUT"}}
            }}
        }}
    }}"#,
        post_port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    let context = HookContext::new("test-node", "req-003");
    let input = Value::String("Input".to_string());
    let output = Value::String("LLM output".to_string());

    let result = hook_executor
        .execute_post_hooks(&node.hooks, &input, output.clone(), &context)
        .await
        .unwrap();

    sleep(Duration::from_millis(100)).await;

    // Observe mode: output unchanged
    assert_eq!(result, output);
    assert!(call_count.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn test_post_hook_transform_mode() {
    let post_port = find_available_port();
    let call_count = start_post_hook_server(post_port, Some("VALIDATED".to_string())).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "validate-output".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", post_port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"output": "$OUTPUT"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "post": [
                        {{"function": "validate-output", "mode": "transform"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "validate-output": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"output": "$OUTPUT"}}
            }}
        }}
    }}"#,
        post_port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    let context = HookContext::new("test-node", "req-004");
    let input = Value::String("Input".to_string());
    let output = Value::String("LLM output".to_string());

    let result = hook_executor
        .execute_post_hooks(&node.hooks, &input, output, &context)
        .await
        .unwrap();

    // Transform mode should modify output
    assert_eq!(
        result,
        Value::String("LLM output | VALIDATED".to_string())
    );
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Failure Handling Tests
// ============================================================================

#[tokio::test]
async fn test_hook_failure_continue() {
    let fail_port = find_available_port();
    let call_count = start_failing_hook_server(fail_port).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "failing-hook".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", fail_port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"input": "$INPUT"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "pre": [
                        {{"function": "failing-hook", "mode": "transform", "on_failure": "continue"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "failing-hook": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"input": "$INPUT"}}
            }}
        }}
    }}"#,
        fail_port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    let context = HookContext::new("test-node", "req-005");
    let input = Value::String("Original input".to_string());

    // With on_failure: continue, execution should proceed despite failure
    let result = hook_executor
        .execute_pre_hooks(&node.hooks, input.clone(), &context)
        .await
        .unwrap();

    // Input should remain unchanged after failure with continue
    assert_eq!(result, input);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_hook_failure_abort() {
    let fail_port = find_available_port();
    let call_count = start_failing_hook_server(fail_port).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "failing-hook".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", fail_port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"input": "$INPUT"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "pre": [
                        {{"function": "failing-hook", "mode": "transform", "on_failure": "abort"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "failing-hook": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"input": "$INPUT"}}
            }}
        }}
    }}"#,
        fail_port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    let context = HookContext::new("test-node", "req-006");
    let input = Value::String("Original input".to_string());

    // With on_failure: abort, execution should return an error
    let result = hook_executor
        .execute_pre_hooks(&node.hooks, input, &context)
        .await;

    assert!(result.is_err());
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Multiple Hooks Chain Test
// ============================================================================

#[tokio::test]
async fn test_multiple_transform_hooks_chain() {
    let port1 = find_available_port();
    let port2 = find_available_port();

    let call_count1 = start_post_hook_server(port1, Some("STEP1".to_string())).await;
    let call_count2 = start_post_hook_server(port2, Some("STEP2".to_string())).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "step1".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", port1),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"output": "$OUTPUT"})),
            timeout: 5,
        },
    );
    functions.insert(
        "step2".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", port2),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"output": "$OUTPUT"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "post": [
                        {{"function": "step1", "mode": "transform"}},
                        {{"function": "step2", "mode": "transform"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "step1": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"output": "$OUTPUT"}}
            }},
            "step2": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"output": "$OUTPUT"}}
            }}
        }}
    }}"#,
        port1, port2
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    let context = HookContext::new("test-node", "req-007");
    let input = Value::String("Input".to_string());
    let output = Value::String("START".to_string());

    let result = hook_executor
        .execute_post_hooks(&node.hooks, &input, output, &context)
        .await
        .unwrap();

    // Hooks should chain: START -> START | STEP1 -> START | STEP1 | STEP2
    assert_eq!(
        result,
        Value::String("START | STEP1 | STEP2".to_string())
    );
    assert_eq!(call_count1.load(Ordering::SeqCst), 1);
    assert_eq!(call_count2.load(Ordering::SeqCst), 1);
}

// ============================================================================
// Conditional Hook Execution Test
// ============================================================================

#[tokio::test]
async fn test_hook_with_condition() {
    let port = find_available_port();
    let call_count = start_pre_hook_server(port, Some("TRANSFORMED".to_string())).await;

    let mut functions = std::collections::HashMap::new();
    functions.insert(
        "conditional-hook".to_string(),
        FunctionType::Rest {
            method: llmnet::config::HttpMethod::Post,
            url: format!("http://127.0.0.1:{}/hook", port),
            headers: std::collections::HashMap::new(),
            body: Some(json!({"input": "$INPUT"})),
            timeout: 5,
        },
    );

    let secrets = Arc::new(SecretsManager::new());
    let function_executor = Arc::new(FunctionExecutor::new(secrets));
    let hook_executor = HookExecutor::new(function_executor, functions);

    let json = format!(
        r#"{{
        "models": {{}},
        "architecture": [
            {{
                "name": "test-node",
                "layer": 0,
                "adapter": "openai-api",
                "hooks": {{
                    "pre": [
                        {{"function": "conditional-hook", "mode": "transform", "if": "$SHOULD_TRANSFORM"}}
                    ]
                }}
            }},
            {{"name": "output", "adapter": "output"}}
        ],
        "functions": {{
            "conditional-hook": {{
                "type": "rest",
                "method": "POST",
                "url": "http://127.0.0.1:{}/hook",
                "body": {{"input": "$INPUT"}}
            }}
        }}
    }}"#,
        port
    );

    let comp = Composition::from_str(&json).unwrap();
    let node = &comp.architecture[0];

    // Test when condition is NOT met
    let context_no_var = HookContext::new("test-node", "req-008");
    let input = Value::String("Original".to_string());

    let result = hook_executor
        .execute_pre_hooks(&node.hooks, input.clone(), &context_no_var)
        .await
        .unwrap();

    // Condition not met, hook should be skipped
    assert_eq!(result, input);
    assert_eq!(call_count.load(Ordering::SeqCst), 0);

    // Test when condition IS met
    let mut context_with_var = HookContext::new("test-node", "req-009");
    context_with_var
        .custom_vars
        .insert("SHOULD_TRANSFORM".to_string(), Value::Bool(true));

    let result = hook_executor
        .execute_pre_hooks(&node.hooks, input, &context_with_var)
        .await
        .unwrap();

    // Condition met, hook should run
    assert_eq!(result, Value::String("TRANSFORMED".to_string()));
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}
