use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::architecture::{ArchitectureNode, OutputTarget};
use super::functions::FunctionType;
use super::models::ModelDefinition;
use super::secrets::SecretSource;

/// Errors that can occur during composition parsing and validation
#[derive(Error, Debug, PartialEq)]
pub enum CompositionError {
    #[error("JSON parse error: {0}")]
    ParseError(String),

    #[error("Model '{0}' referenced by node '{1}' is not defined")]
    UndefinedModel(String, String),

    #[error("Node '{0}' referenced in output-to is not defined")]
    UndefinedNode(String),

    #[error("No router node (layer 0) found in architecture")]
    NoRouterNode,

    #[error("No output node found in architecture")]
    NoOutputNode,

    #[error("Duplicate node name: '{0}'")]
    DuplicateNodeName(String),

    #[error("Function '{0}' referenced by hook in node '{1}' is not defined")]
    UndefinedFunction(String, String),
}

/// The complete composition file structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Composition {
    pub models: HashMap<String, ModelDefinition>,
    pub architecture: Vec<ArchitectureNode>,
    /// Secret sources for credential management
    #[serde(default)]
    pub secrets: HashMap<String, SecretSource>,
    /// Reusable function definitions for hooks
    #[serde(default)]
    pub functions: HashMap<String, FunctionType>,
}

// ============================================================================
// SBIO: Pure parsing functions (no I/O)
// ============================================================================

/// Strip C-style comments from JSONC content.
/// This is a pure function - no I/O.
pub fn strip_jsonc_comments(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == '/' {
            if chars.peek() == Some(&'/') {
                // Line comment - skip until newline
                chars.next(); // consume second /
                for nc in chars.by_ref() {
                    if nc == '\n' {
                        result.push('\n');
                        break;
                    }
                }
                continue;
            } else if chars.peek() == Some(&'*') {
                // Block comment - skip until */
                chars.next(); // consume *
                let mut prev = ' ';
                for nc in chars.by_ref() {
                    if prev == '*' && nc == '/' {
                        break;
                    }
                    prev = nc;
                }
                continue;
            }
        }

        result.push(c);
    }

    result
}

/// Parse a JSONC string into a Composition.
/// This is a pure function - no I/O.
pub fn parse_composition(content: &str) -> Result<Composition, CompositionError> {
    let stripped = strip_jsonc_comments(content);
    serde_json::from_str(&stripped).map_err(|e| CompositionError::ParseError(e.to_string()))
}

/// Validate a composition for consistency.
/// This is a pure function - no I/O.
pub fn validate_composition(composition: &Composition) -> Result<(), CompositionError> {
    let node_names: HashMap<_, _> = composition
        .architecture
        .iter()
        .map(|n| (&n.name, n))
        .collect();

    // Check for duplicate node names
    if node_names.len() != composition.architecture.len() {
        let mut seen = std::collections::HashSet::new();
        for node in &composition.architecture {
            if !seen.insert(&node.name) {
                return Err(CompositionError::DuplicateNodeName(node.name.clone()));
            }
        }
    }

    // Check that all model references exist
    for node in &composition.architecture {
        if let Some(model_ref) = &node.model {
            if !composition.models.contains_key(model_ref) {
                return Err(CompositionError::UndefinedModel(
                    model_ref.clone(),
                    node.name.clone(),
                ));
            }
        }
    }

    // Check that output-to node references exist
    for node in &composition.architecture {
        if let Some(OutputTarget::Nodes(targets)) = &node.output_to {
            for target in targets {
                if !node_names.contains_key(target) {
                    return Err(CompositionError::UndefinedNode(target.clone()));
                }
            }
        }
    }

    // Check for at least one router node (layer 0)
    let has_router = composition
        .architecture
        .iter()
        .any(|n| n.layer == Some(0));
    if !has_router {
        return Err(CompositionError::NoRouterNode);
    }

    // Check for at least one output node
    let has_output = composition.architecture.iter().any(|n| n.is_output());
    if !has_output {
        return Err(CompositionError::NoOutputNode);
    }

    // Check that all hook function references exist
    for node in &composition.architecture {
        for hook in &node.hooks.pre {
            if !composition.functions.contains_key(&hook.function) {
                return Err(CompositionError::UndefinedFunction(
                    hook.function.clone(),
                    node.name.clone(),
                ));
            }
        }
        for hook in &node.hooks.post {
            if !composition.functions.contains_key(&hook.function) {
                return Err(CompositionError::UndefinedFunction(
                    hook.function.clone(),
                    node.name.clone(),
                ));
            }
        }
    }

    Ok(())
}

impl Composition {
    /// Parse and validate from a JSONC string.
    /// Pure function - no I/O.
    pub fn from_str(content: &str) -> Result<Self, CompositionError> {
        let composition = parse_composition(content)?;
        validate_composition(&composition)?;
        Ok(composition)
    }

    /// Get all nodes in a specific layer
    pub fn nodes_in_layer(&self, layer: u32) -> Vec<&ArchitectureNode> {
        self.architecture
            .iter()
            .filter(|n| n.layer == Some(layer))
            .collect()
    }

    /// Get the router node (layer 0)
    pub fn router_node(&self) -> Option<&ArchitectureNode> {
        self.architecture.iter().find(|n| n.layer == Some(0))
    }

    /// Get a node by name
    pub fn node_by_name(&self, name: &str) -> Option<&ArchitectureNode> {
        self.architecture.iter().find(|n| n.name == name)
    }

    /// Get the model definition for a node
    pub fn model_for_node(&self, node: &ArchitectureNode) -> Option<&ModelDefinition> {
        node.model.as_ref().and_then(|m| self.models.get(m))
    }

    /// Get all output nodes
    pub fn output_nodes(&self) -> Vec<&ArchitectureNode> {
        self.architecture.iter().filter(|n| n.is_output()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_line_comments() {
        let input = r#"{
            // This is a comment
            "key": "value"
        }"#;
        let result = strip_jsonc_comments(input);
        assert!(!result.contains("This is a comment"));
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_strip_block_comments() {
        let input = r#"{
            /* Block comment */
            "key": "value"
        }"#;
        let result = strip_jsonc_comments(input);
        assert!(!result.contains("Block comment"));
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_preserve_strings_with_slashes() {
        let input = r#"{"url": "http://example.com"}"#;
        let result = strip_jsonc_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_preserve_strings_with_comment_like_content() {
        let input = r#"{"desc": "use // for comments"}"#;
        let result = strip_jsonc_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_parse_minimal_composition() {
        let json = r#"{
            "models": {
                "test-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "test-model",
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "final-output",
                    "adapter": "output"
                }
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        assert_eq!(comp.models.len(), 1);
        assert_eq!(comp.architecture.len(), 2);
    }

    #[test]
    fn test_parse_with_comments() {
        let jsonc = r#"{
            // Define models
            "models": {
                "test-model": {
                    "type": "external",
                    "interface": "openai-api",
                    "url": "http://localhost:8080"
                }
            },
            /* Architecture definition */
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "test-model",
                    "adapter": "openai-api",
                    "output-to": [1]
                },
                {
                    "name": "final-output",
                    "adapter": "output"
                }
            ]
        }"#;

        let comp = Composition::from_str(jsonc).unwrap();
        assert_eq!(comp.models.len(), 1);
    }

    #[test]
    fn test_validate_undefined_model() {
        let json = r#"{
            "models": {},
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "model": "nonexistent",
                    "adapter": "openai-api"
                },
                {
                    "name": "final-output",
                    "adapter": "output"
                }
            ]
        }"#;

        let result = Composition::from_str(json);
        assert!(matches!(result, Err(CompositionError::UndefinedModel(_, _))));
    }

    #[test]
    fn test_validate_undefined_node() {
        let json = r#"{
            "models": {},
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "adapter": "openai-api",
                    "output-to": ["nonexistent-node"]
                },
                {
                    "name": "final-output",
                    "adapter": "output"
                }
            ]
        }"#;

        let result = Composition::from_str(json);
        assert!(matches!(result, Err(CompositionError::UndefinedNode(_))));
    }

    #[test]
    fn test_validate_no_router() {
        let json = r#"{
            "models": {},
            "architecture": [
                {
                    "name": "final-output",
                    "adapter": "output"
                }
            ]
        }"#;

        let result = Composition::from_str(json);
        assert!(matches!(result, Err(CompositionError::NoRouterNode)));
    }

    #[test]
    fn test_validate_no_output() {
        let json = r#"{
            "models": {},
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "adapter": "openai-api"
                }
            ]
        }"#;

        let result = Composition::from_str(json);
        assert!(matches!(result, Err(CompositionError::NoOutputNode)));
    }

    #[test]
    fn test_nodes_in_layer() {
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "node1", "layer": 1, "adapter": "openai-api"},
                {"name": "node2", "layer": 1, "adapter": "openai-api"},
                {"name": "final-output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let layer1 = comp.nodes_in_layer(1);
        assert_eq!(layer1.len(), 2);
    }

    #[test]
    fn test_router_node() {
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "my-router", "layer": 0, "adapter": "openai-api"},
                {"name": "final-output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        let router = comp.router_node().unwrap();
        assert_eq!(router.name, "my-router");
    }

    #[test]
    fn test_node_by_name() {
        let json = r#"{
            "models": {},
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "final-output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        assert!(comp.node_by_name("router").is_some());
        assert!(comp.node_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_parse_with_secrets_and_functions() {
        let json = r#"{
            "models": {},
            "secrets": {
                "api-keys": {
                    "source": "env-file",
                    "path": "~/.secrets/.env",
                    "variables": ["API_KEY"]
                }
            },
            "functions": {
                "log-request": {
                    "type": "rest",
                    "method": "POST",
                    "url": "https://api.example.com/log"
                }
            },
            "architecture": [
                {"name": "router", "layer": 0, "adapter": "openai-api"},
                {"name": "final-output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json).unwrap();
        assert_eq!(comp.secrets.len(), 1);
        assert!(comp.secrets.contains_key("api-keys"));
        assert_eq!(comp.functions.len(), 1);
        assert!(comp.functions.contains_key("log-request"));
    }

    #[test]
    fn test_validate_undefined_function() {
        let json = r#"{
            "models": {},
            "functions": {},
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "adapter": "openai-api",
                    "hooks": {
                        "pre": [{"function": "nonexistent"}]
                    }
                },
                {"name": "final-output", "adapter": "output"}
            ]
        }"#;

        let result = Composition::from_str(json);
        assert!(matches!(
            result,
            Err(CompositionError::UndefinedFunction(name, _)) if name == "nonexistent"
        ));
    }

    #[test]
    fn test_valid_hook_function_reference() {
        let json = r#"{
            "models": {},
            "functions": {
                "my-hook": {
                    "type": "shell",
                    "command": "echo",
                    "args": ["hello"]
                }
            },
            "architecture": [
                {
                    "name": "router",
                    "layer": 0,
                    "adapter": "openai-api",
                    "hooks": {
                        "post": [{"function": "my-hook"}]
                    }
                },
                {"name": "final-output", "adapter": "output"}
            ]
        }"#;

        let comp = Composition::from_str(json);
        assert!(comp.is_ok());
    }
}
