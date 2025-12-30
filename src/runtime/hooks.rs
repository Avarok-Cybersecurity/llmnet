//! Hook execution engine
//!
//! This module executes pre/post hooks on architecture nodes.
//! Hooks can run in observe mode (fire-and-forget) or transform mode (modifies data).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::config::architecture::{FailureAction, HookConfig, HookMode, NodeHooks};
use crate::config::functions::{FunctionExecutor, FunctionType};

/// Errors during hook execution
#[derive(Error, Debug)]
pub enum HookError {
    #[error("Function not found: {0}")]
    FunctionNotFound(String),

    #[error("Hook execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Hook aborted: {0}")]
    Aborted(String),

    #[error("Condition evaluation failed: {0}")]
    ConditionError(String),
}

/// Hook execution context with variables
#[derive(Debug, Clone)]
pub struct HookContext {
    pub node_name: String,
    pub prev_node: Option<String>,
    pub request_id: String,
    pub timestamp: String,
    pub custom_vars: HashMap<String, Value>,
}

impl HookContext {
    pub fn new(node_name: &str, request_id: &str) -> Self {
        Self {
            node_name: node_name.to_string(),
            prev_node: None,
            request_id: request_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            custom_vars: HashMap::new(),
        }
    }

    /// Build variables map for function execution
    pub fn build_variables(&self, input: &Value, output: Option<&Value>) -> HashMap<String, Value> {
        let mut vars = HashMap::new();

        vars.insert("NODE".to_string(), Value::String(self.node_name.clone()));
        vars.insert("REQUEST_ID".to_string(), Value::String(self.request_id.clone()));
        vars.insert("TIMESTAMP".to_string(), Value::String(self.timestamp.clone()));
        vars.insert("INPUT".to_string(), input.clone());

        if let Some(prev) = &self.prev_node {
            vars.insert("PREV_NODE".to_string(), Value::String(prev.clone()));
        }

        if let Some(out) = output {
            vars.insert("OUTPUT".to_string(), out.clone());
        }

        // Merge custom variables
        for (k, v) in &self.custom_vars {
            vars.insert(k.clone(), v.clone());
        }

        vars
    }
}

/// Executor for running hooks
pub struct HookExecutor {
    function_executor: Arc<FunctionExecutor>,
    functions: HashMap<String, FunctionType>,
}

impl HookExecutor {
    pub fn new(
        function_executor: Arc<FunctionExecutor>,
        functions: HashMap<String, FunctionType>,
    ) -> Self {
        Self {
            function_executor,
            functions,
        }
    }

    /// Execute pre-hooks before node processing
    /// Returns modified input if any transform hooks succeed
    pub async fn execute_pre_hooks(
        &self,
        hooks: &NodeHooks,
        input: Value,
        context: &HookContext,
    ) -> Result<Value, HookError> {
        self.execute_hooks(&hooks.pre, input, None, context).await
    }

    /// Execute post-hooks after node processing
    /// Returns modified output if any transform hooks succeed
    pub async fn execute_post_hooks(
        &self,
        hooks: &NodeHooks,
        input: &Value,
        output: Value,
        context: &HookContext,
    ) -> Result<Value, HookError> {
        self.execute_hooks(&hooks.post, output, Some(input), context)
            .await
    }

    /// Execute a list of hooks
    async fn execute_hooks(
        &self,
        hooks: &[HookConfig],
        mut data: Value,
        input_for_context: Option<&Value>,
        context: &HookContext,
    ) -> Result<Value, HookError> {
        for hook in hooks {
            // Check condition if present
            if let Some(condition) = &hook.condition {
                let vars = context.build_variables(
                    input_for_context.unwrap_or(&data),
                    Some(&data),
                );
                if !self.evaluate_condition(condition, &vars) {
                    debug!(
                        "Skipping hook '{}' on node '{}': condition not met",
                        hook.function, context.node_name
                    );
                    continue;
                }
            }

            // Get function config
            let func = self.functions.get(&hook.function).ok_or_else(|| {
                HookError::FunctionNotFound(hook.function.clone())
            })?;

            // Build variables
            let vars = context.build_variables(
                input_for_context.unwrap_or(&data),
                Some(&data),
            );

            match hook.mode {
                HookMode::Observe => {
                    // Fire and forget - spawn task
                    let executor = self.function_executor.clone();
                    let func = func.clone();
                    let vars = vars.clone();
                    let hook_name = hook.function.clone();
                    let node_name = context.node_name.clone();
                    let on_failure = hook.on_failure.clone();

                    tokio::spawn(async move {
                        match executor.execute(&func, &vars).await {
                            Ok(result) => {
                                if result.success {
                                    debug!(
                                        "Observe hook '{}' completed on node '{}' in {}ms",
                                        hook_name, node_name, result.duration_ms
                                    );
                                } else if let Some(err) = result.error {
                                    match on_failure {
                                        FailureAction::Continue => {
                                            warn!(
                                                "Observe hook '{}' failed on node '{}': {}",
                                                hook_name, node_name, err
                                            );
                                        }
                                        FailureAction::Abort => {
                                            error!(
                                                "Observe hook '{}' failed on node '{}' (abort requested but ignored in observe mode): {}",
                                                hook_name, node_name, err
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Observe hook '{}' error on node '{}': {}",
                                    hook_name, node_name, e
                                );
                            }
                        }
                    });
                }
                HookMode::Transform => {
                    // Wait for result and potentially modify data
                    let result = self
                        .function_executor
                        .execute(func, &vars)
                        .await
                        .map_err(|e| HookError::ExecutionFailed(e.to_string()))?;

                    if result.success {
                        info!(
                            "Transform hook '{}' completed on node '{}' in {}ms",
                            hook.function, context.node_name, result.duration_ms
                        );

                        // If hook returned output, use it to transform data
                        if let Some(output) = result.output {
                            data = self.apply_transform(data, output);
                        }
                    } else {
                        let err_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                        match hook.on_failure {
                            FailureAction::Continue => {
                                warn!(
                                    "Transform hook '{}' failed on node '{}': {}",
                                    hook.function, context.node_name, err_msg
                                );
                            }
                            FailureAction::Abort => {
                                return Err(HookError::Aborted(format!(
                                    "Hook '{}' failed: {}",
                                    hook.function, err_msg
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(data)
    }

    /// Evaluate a condition expression
    /// This is a simplified evaluator - complex conditions should use the node.rs evaluator
    fn evaluate_condition(&self, condition: &str, variables: &HashMap<String, Value>) -> bool {
        // Simple variable presence check: $VAR or $VAR.field
        if condition.starts_with('$') && !condition.contains(' ') {
            let var_path = &condition[1..]; // Remove $
            let parts: Vec<&str> = var_path.split('.').collect();

            if parts.is_empty() {
                return false;
            }

            let mut current = variables.get(parts[0]);
            for part in &parts[1..] {
                match current {
                    Some(Value::Object(obj)) => {
                        current = obj.get(*part);
                    }
                    _ => return false,
                }
            }

            // Check if value is truthy
            match current {
                Some(Value::Bool(b)) => *b,
                Some(Value::Null) => false,
                Some(Value::String(s)) => !s.is_empty(),
                Some(Value::Number(_)) => true,
                Some(Value::Array(a)) => !a.is_empty(),
                Some(Value::Object(o)) => !o.is_empty(),
                None => false,
            }
        } else if condition.contains("==") {
            // Simple equality check: $VAR == "value"
            let parts: Vec<&str> = condition.split("==").collect();
            if parts.len() != 2 {
                return false;
            }

            let left = parts[0].trim();
            let right = parts[1].trim().trim_matches('"').trim_matches('\'');

            if left.starts_with('$') {
                let var_name = &left[1..];
                match variables.get(var_name) {
                    Some(Value::String(s)) => s == right,
                    Some(Value::Bool(b)) => {
                        (right == "true" && *b) || (right == "false" && !*b)
                    }
                    Some(Value::Number(n)) => n.to_string() == right,
                    _ => false,
                }
            } else {
                false
            }
        } else if condition.contains("!=") {
            // Simple inequality check
            let parts: Vec<&str> = condition.split("!=").collect();
            if parts.len() != 2 {
                return false;
            }

            let left = parts[0].trim();
            let right = parts[1].trim().trim_matches('"').trim_matches('\'');

            if left.starts_with('$') {
                let var_name = &left[1..];
                match variables.get(var_name) {
                    Some(Value::String(s)) => s != right,
                    Some(Value::Bool(b)) => {
                        !((right == "true" && *b) || (right == "false" && !*b))
                    }
                    Some(Value::Number(n)) => n.to_string() != right,
                    _ => true, // Not found != anything is true
                }
            } else {
                false
            }
        } else {
            // Default: treat as truthy check
            !condition.is_empty()
        }
    }

    /// Apply transform output to data
    /// If output is an object with specific keys, merge it
    /// Otherwise, replace entirely
    fn apply_transform(&self, current: Value, transform: Value) -> Value {
        match (&current, &transform) {
            (Value::Object(curr_obj), Value::Object(trans_obj)) => {
                // Merge objects
                let mut result = curr_obj.clone();
                for (k, v) in trans_obj {
                    result.insert(k.clone(), v.clone());
                }
                Value::Object(result)
            }
            _ => transform, // Replace entirely
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_context() -> HookContext {
        HookContext::new("test-node", "req-123")
    }

    #[test]
    fn test_hook_context_build_variables() {
        let mut ctx = create_test_context();
        ctx.prev_node = Some("prev-node".to_string());
        ctx.custom_vars.insert("CUSTOM".to_string(), Value::String("value".to_string()));

        let input = Value::String("input data".to_string());
        let output = Value::String("output data".to_string());

        let vars = ctx.build_variables(&input, Some(&output));

        assert_eq!(vars.get("NODE"), Some(&Value::String("test-node".to_string())));
        assert_eq!(vars.get("PREV_NODE"), Some(&Value::String("prev-node".to_string())));
        assert_eq!(vars.get("INPUT"), Some(&Value::String("input data".to_string())));
        assert_eq!(vars.get("OUTPUT"), Some(&Value::String("output data".to_string())));
        assert_eq!(vars.get("CUSTOM"), Some(&Value::String("value".to_string())));
        assert!(vars.contains_key("REQUEST_ID"));
        assert!(vars.contains_key("TIMESTAMP"));
    }

    #[test]
    fn test_evaluate_condition_variable_presence() {
        let mut vars = HashMap::new();
        vars.insert("FLAG".to_string(), Value::Bool(true));
        vars.insert("EMPTY".to_string(), Value::String("".to_string()));
        vars.insert("DATA".to_string(), Value::String("hello".to_string()));

        let secrets = Arc::new(crate::config::secrets::SecretsManager::new());
        let executor = Arc::new(FunctionExecutor::new(secrets));
        let hook_executor = HookExecutor::new(executor, HashMap::new());

        assert!(hook_executor.evaluate_condition("$FLAG", &vars));
        assert!(!hook_executor.evaluate_condition("$EMPTY", &vars));
        assert!(hook_executor.evaluate_condition("$DATA", &vars));
        assert!(!hook_executor.evaluate_condition("$MISSING", &vars));
    }

    #[test]
    fn test_evaluate_condition_equality() {
        let mut vars = HashMap::new();
        vars.insert("STATUS".to_string(), Value::String("ok".to_string()));
        vars.insert("COUNT".to_string(), Value::Number(42.into()));

        let secrets = Arc::new(crate::config::secrets::SecretsManager::new());
        let executor = Arc::new(FunctionExecutor::new(secrets));
        let hook_executor = HookExecutor::new(executor, HashMap::new());

        assert!(hook_executor.evaluate_condition("$STATUS == \"ok\"", &vars));
        assert!(!hook_executor.evaluate_condition("$STATUS == \"error\"", &vars));
        assert!(hook_executor.evaluate_condition("$COUNT == 42", &vars));
    }

    #[test]
    fn test_evaluate_condition_inequality() {
        let mut vars = HashMap::new();
        vars.insert("STATUS".to_string(), Value::String("ok".to_string()));

        let secrets = Arc::new(crate::config::secrets::SecretsManager::new());
        let executor = Arc::new(FunctionExecutor::new(secrets));
        let hook_executor = HookExecutor::new(executor, HashMap::new());

        assert!(hook_executor.evaluate_condition("$STATUS != \"error\"", &vars));
        assert!(!hook_executor.evaluate_condition("$STATUS != \"ok\"", &vars));
    }

    #[test]
    fn test_apply_transform_merge_objects() {
        let secrets = Arc::new(crate::config::secrets::SecretsManager::new());
        let executor = Arc::new(FunctionExecutor::new(secrets));
        let hook_executor = HookExecutor::new(executor, HashMap::new());

        let current = serde_json::json!({"a": 1, "b": 2});
        let transform = serde_json::json!({"b": 3, "c": 4});

        let result = hook_executor.apply_transform(current, transform);
        assert_eq!(result["a"], 1);
        assert_eq!(result["b"], 3); // Updated
        assert_eq!(result["c"], 4); // Added
    }

    #[test]
    fn test_apply_transform_replace() {
        let secrets = Arc::new(crate::config::secrets::SecretsManager::new());
        let executor = Arc::new(FunctionExecutor::new(secrets));
        let hook_executor = HookExecutor::new(executor, HashMap::new());

        let current = Value::String("old".to_string());
        let transform = Value::String("new".to_string());

        let result = hook_executor.apply_transform(current, transform);
        assert_eq!(result, Value::String("new".to_string()));
    }
}
