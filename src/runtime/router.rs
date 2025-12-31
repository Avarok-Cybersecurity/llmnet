use thiserror::Error;

use crate::client::{ChatCompletionRequest, ClientError, Message, OpenAiClientTrait};
use crate::config::ArchitectureNode;

#[derive(Error, Debug)]
pub enum RouterError {
    #[error("Client error: {0}")]
    Client(#[from] ClientError),

    #[error("No valid node selected from response: '{0}'")]
    InvalidSelection(String),

    #[error("Empty response from router model")]
    EmptyResponse,
}

/// Metadata about a node that the router uses for decision-making
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeMetadata {
    pub name: String,
    #[serde(rename = "use-case")]
    pub use_case: Option<String>,
}

impl From<&ArchitectureNode> for NodeMetadata {
    fn from(node: &ArchitectureNode) -> Self {
        Self {
            name: node.name.clone(),
            use_case: node.use_case.clone(),
        }
    }
}

// ============================================================================
// SBIO: Pure functions for prompt generation and response parsing
// ============================================================================

/// Build the routing prompt for the router model.
/// Pure function - no I/O.
pub fn build_routing_prompt(user_prompt: &str, available_nodes: &[NodeMetadata]) -> String {
    let nodes_json =
        serde_json::to_string_pretty(available_nodes).unwrap_or_else(|_| "[]".to_string());

    format!(
        "Here is the user prompt: {}\n\n\
         Based on the prompt, please choose from one of these models, \
         outputting ONLY the model name to use:\n{}",
        user_prompt, nodes_json
    )
}

/// Extract the selected node name from the router's response.
/// Pure function - no I/O.
pub fn extract_node_selection(
    response: &str,
    available_nodes: &[NodeMetadata],
) -> Result<String, RouterError> {
    let response = response.trim();

    if response.is_empty() {
        return Err(RouterError::EmptyResponse);
    }

    // First, try exact match
    for node in available_nodes {
        if response == node.name {
            return Ok(node.name.clone());
        }
    }

    // Try case-insensitive match
    let response_lower = response.to_lowercase();
    for node in available_nodes {
        if response_lower == node.name.to_lowercase() {
            return Ok(node.name.clone());
        }
    }

    // Try to find a node name contained in the response
    for node in available_nodes {
        if response.contains(&node.name) {
            return Ok(node.name.clone());
        }
    }

    // Try case-insensitive containment
    for node in available_nodes {
        if response_lower.contains(&node.name.to_lowercase()) {
            return Ok(node.name.clone());
        }
    }

    Err(RouterError::InvalidSelection(response.to_string()))
}

// ============================================================================
// SBIO: Router struct with I/O (uses trait abstraction)
// ============================================================================

/// Router that uses an LLM to decide which node to route to
pub struct Router<C: OpenAiClientTrait> {
    client: C,
    model: String,
}

impl<C: OpenAiClientTrait> Router<C> {
    pub fn new(client: C, model: String) -> Self {
        Self { client, model }
    }

    /// Route a prompt to the appropriate node
    pub async fn route(
        &self,
        prompt: &str,
        available_nodes: &[NodeMetadata],
    ) -> Result<String, RouterError> {
        let routing_prompt = build_routing_prompt(prompt, available_nodes);

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: routing_prompt,
            }],
            max_tokens: Some(100),
            temperature: Some(0.1), // Low temperature for consistent routing
        };

        let response = self.client.chat_completion(&request).await?;

        let content = response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        extract_node_selection(&content, available_nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_nodes() -> Vec<NodeMetadata> {
        vec![
            NodeMetadata {
                name: "company-2024-q3".to_string(),
                use_case: Some("Handle Q3 2024 company queries".to_string()),
            },
            NodeMetadata {
                name: "company-2024-q4".to_string(),
                use_case: Some("Handle Q4 2024 company queries".to_string()),
            },
            NodeMetadata {
                name: "general-assistant".to_string(),
                use_case: Some("General purpose assistant".to_string()),
            },
        ]
    }

    #[test]
    fn test_build_routing_prompt() {
        let nodes = sample_nodes();
        let prompt = build_routing_prompt("What were our Q3 earnings?", &nodes);

        assert!(prompt.contains("What were our Q3 earnings?"));
        assert!(prompt.contains("company-2024-q3"));
        assert!(prompt.contains("company-2024-q4"));
        assert!(prompt.contains("outputting ONLY the model name"));
    }

    #[test]
    fn test_extract_exact_match() {
        let nodes = sample_nodes();
        let result = extract_node_selection("company-2024-q3", &nodes);
        assert_eq!(result.unwrap(), "company-2024-q3");
    }

    #[test]
    fn test_extract_case_insensitive() {
        let nodes = sample_nodes();
        let result = extract_node_selection("COMPANY-2024-Q3", &nodes);
        assert_eq!(result.unwrap(), "company-2024-q3");
    }

    #[test]
    fn test_extract_with_whitespace() {
        let nodes = sample_nodes();
        let result = extract_node_selection("  company-2024-q3  \n", &nodes);
        assert_eq!(result.unwrap(), "company-2024-q3");
    }

    #[test]
    fn test_extract_contained_in_response() {
        let nodes = sample_nodes();
        let result = extract_node_selection("I think company-2024-q3 is the best choice.", &nodes);
        assert_eq!(result.unwrap(), "company-2024-q3");
    }

    #[test]
    fn test_extract_invalid_selection() {
        let nodes = sample_nodes();
        let result = extract_node_selection("some-other-model", &nodes);
        assert!(matches!(result, Err(RouterError::InvalidSelection(_))));
    }

    #[test]
    fn test_extract_empty_response() {
        let nodes = sample_nodes();
        let result = extract_node_selection("", &nodes);
        assert!(matches!(result, Err(RouterError::EmptyResponse)));
    }

    #[tokio::test]
    async fn test_router_with_mock() {
        use crate::client::openai::mock::MockOpenAiClient;

        let mock = MockOpenAiClient::new(vec!["company-2024-q3".to_string()]);
        let router = Router::new(mock, "test-model".to_string());
        let nodes = sample_nodes();

        let result = router
            .route("What were our Q3 2024 results?", &nodes)
            .await
            .unwrap();

        assert_eq!(result, "company-2024-q3");
    }
}
