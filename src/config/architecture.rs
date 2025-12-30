use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Hook configuration types
// ============================================================================

/// Hook execution mode
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HookMode {
    /// Fire-and-forget - doesn't affect pipeline data
    #[default]
    Observe,
    /// Result can modify pipeline input/output
    Transform,
}

/// Action to take when a hook fails
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FailureAction {
    /// Log error and continue execution
    #[default]
    Continue,
    /// Stop pipeline execution
    Abort,
}

/// Configuration for a single hook
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HookConfig {
    /// Name of the function to call
    pub function: String,

    /// Execution mode
    #[serde(default)]
    pub mode: HookMode,

    /// What to do on failure
    #[serde(default, rename = "on_failure")]
    pub on_failure: FailureAction,

    /// Optional condition for hook execution (uses same syntax as node conditions)
    #[serde(rename = "if", skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// Pre and post hooks for an architecture node
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub struct NodeHooks {
    /// Hooks executed before node processing
    #[serde(default)]
    pub pre: Vec<HookConfig>,

    /// Hooks executed after node processing
    #[serde(default)]
    pub post: Vec<HookConfig>,
}

// ============================================================================
// Architecture node definition
// ============================================================================

/// Architecture node definition from the composition file
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ArchitectureNode {
    pub name: String,

    #[serde(default)]
    pub layer: Option<u32>,

    /// Reference to a model in the models map
    pub model: Option<String>,

    /// Adapter type: "openai-api", "output", "ws"
    pub adapter: String,

    #[serde(rename = "bind-addr")]
    pub bind_addr: Option<String>,

    #[serde(rename = "bind-port")]
    pub bind_port: Option<String>,

    #[serde(rename = "output-to")]
    pub output_to: Option<OutputTarget>,

    /// Use-case description for routing decisions
    #[serde(rename = "use-case")]
    pub use_case: Option<String>,

    /// Conditional execution expression
    #[serde(rename = "if")]
    pub condition: Option<String>,

    /// WebSocket URL for "ws" adapter
    pub url: Option<String>,

    /// Deployment context override (default: localhost)
    /// Use to deploy local runners to remote nodes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,

    #[serde(rename = "extra-options", default)]
    pub extra_options: HashMap<String, serde_json::Value>,

    /// Pre and post execution hooks
    #[serde(default)]
    pub hooks: NodeHooks,
}

/// Output target specification - can be layers or specific nodes
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum OutputTarget {
    /// Output to specific layer numbers
    Layers(Vec<u32>),
    /// Output to specific node names
    Nodes(Vec<String>),
}

impl ArchitectureNode {
    /// Check if this is a router node (layer 0 with output-to)
    pub fn is_router(&self) -> bool {
        self.layer == Some(0) && self.output_to.is_some()
    }

    /// Check if this is an output node
    pub fn is_output(&self) -> bool {
        self.adapter == "output"
    }

    /// Get the bind address with default
    pub fn effective_bind_addr(&self) -> &str {
        self.bind_addr.as_deref().unwrap_or("0.0.0.0")
    }

    /// Get nodes in the next layer based on output-to
    pub fn get_target_layer(&self) -> Option<u32> {
        match &self.output_to {
            Some(OutputTarget::Layers(layers)) => layers.first().copied(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_router_node() {
        let json = r#"{
            "name": "router-layer",
            "layer": 0,
            "model": "my-special-input-model",
            "adapter": "openai-api",
            "bind-addr": "0.0.0.0",
            "bind-port": "8080",
            "output-to": [1]
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.name, "router-layer");
        assert_eq!(node.layer, Some(0));
        assert_eq!(node.model, Some("my-special-input-model".to_string()));
        assert!(node.is_router());
        assert_eq!(node.get_target_layer(), Some(1));
    }

    #[test]
    fn test_parse_hidden_layer_node() {
        let json = r#"{
            "name": "company-2024-q3-fine-tune-qwen3-30b",
            "layer": 1,
            "adapter": "openai-api",
            "use-case": "If $INPUT is about company information during 2024 q3",
            "bind-addr": "0.0.0.0",
            "bind-port": "8081",
            "output-to": ["final-output"]
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.layer, Some(1));
        assert_eq!(
            node.use_case,
            Some("If $INPUT is about company information during 2024 q3".to_string())
        );
        match &node.output_to {
            Some(OutputTarget::Nodes(nodes)) => {
                assert_eq!(nodes, &vec!["final-output".to_string()]);
            }
            _ => panic!("Expected Nodes output target"),
        }
    }

    #[test]
    fn test_parse_output_node() {
        let json = r#"{
            "name": "final-output",
            "adapter": "output"
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert!(node.is_output());
        assert!(!node.is_router());
    }

    #[test]
    fn test_parse_conditional_ws_node() {
        let json = r#"{
            "name": "output-to-ws",
            "if": "$OutputCustomKey",
            "adapter": "ws",
            "url": "ws://localhost:3000"
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.condition, Some("$OutputCustomKey".to_string()));
        assert_eq!(node.url, Some("ws://localhost:3000".to_string()));
    }

    #[test]
    fn test_effective_bind_addr_default() {
        let node = ArchitectureNode {
            name: "test".to_string(),
            layer: None,
            model: None,
            adapter: "output".to_string(),
            bind_addr: None,
            bind_port: None,
            output_to: None,
            use_case: None,
            condition: None,
            url: None,
            context: None,
            extra_options: HashMap::new(),
            hooks: NodeHooks::default(),
        };
        assert_eq!(node.effective_bind_addr(), "0.0.0.0");
    }

    #[test]
    fn test_parse_node_with_context() {
        let json = r#"{
            "name": "remote-model",
            "layer": 1,
            "model": "llama-vllm",
            "adapter": "openai-api",
            "context": "gpu-cluster"
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.context, Some("gpu-cluster".to_string()));
    }

    #[test]
    fn test_parse_hook_config() {
        let json = r#"{
            "function": "log-request",
            "mode": "observe",
            "on_failure": "continue"
        }"#;

        let hook: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(hook.function, "log-request");
        assert_eq!(hook.mode, HookMode::Observe);
        assert_eq!(hook.on_failure, FailureAction::Continue);
        assert!(hook.condition.is_none());
    }

    #[test]
    fn test_parse_hook_config_with_condition() {
        let json = r#"{
            "function": "validate-output",
            "mode": "transform",
            "on_failure": "abort",
            "if": "$OUTPUT.valid == true"
        }"#;

        let hook: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(hook.function, "validate-output");
        assert_eq!(hook.mode, HookMode::Transform);
        assert_eq!(hook.on_failure, FailureAction::Abort);
        assert_eq!(hook.condition, Some("$OUTPUT.valid == true".to_string()));
    }

    #[test]
    fn test_parse_hook_config_defaults() {
        let json = r#"{"function": "simple-hook"}"#;

        let hook: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(hook.function, "simple-hook");
        assert_eq!(hook.mode, HookMode::Observe); // default
        assert_eq!(hook.on_failure, FailureAction::Continue); // default
    }

    #[test]
    fn test_parse_node_with_hooks() {
        let json = r#"{
            "name": "router",
            "layer": 0,
            "adapter": "openai-api",
            "hooks": {
                "pre": [
                    {"function": "check-quota", "mode": "transform", "on_failure": "abort"}
                ],
                "post": [
                    {"function": "log-request", "mode": "observe"},
                    {"function": "validate-output", "mode": "transform", "on_failure": "abort"}
                ]
            }
        }"#;

        let node: ArchitectureNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.hooks.pre.len(), 1);
        assert_eq!(node.hooks.post.len(), 2);
        assert_eq!(node.hooks.pre[0].function, "check-quota");
        assert_eq!(node.hooks.pre[0].mode, HookMode::Transform);
        assert_eq!(node.hooks.post[0].function, "log-request");
        assert_eq!(node.hooks.post[0].mode, HookMode::Observe);
    }

    #[test]
    fn test_node_hooks_default() {
        let hooks = NodeHooks::default();
        assert!(hooks.pre.is_empty());
        assert!(hooks.post.is_empty());
    }
}
