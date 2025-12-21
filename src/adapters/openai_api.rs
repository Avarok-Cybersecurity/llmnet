use async_trait::async_trait;
use thiserror::Error;

use crate::client::{ChatCompletionRequest, ClientError, Message, OpenAiClient, OpenAiClientTrait};
use crate::runtime::PipelineRequest;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Client error: {0}")]
    Client(#[from] ClientError),
}

/// Trait for adapters that process requests
#[async_trait]
pub trait Adapter: Send + Sync {
    async fn process(&self, request: &PipelineRequest) -> Result<String, AdapterError>;
}

/// OpenAI API adapter for forwarding requests to OpenAI-compatible endpoints
pub struct OpenAiApiAdapter {
    client: OpenAiClient,
}

impl OpenAiApiAdapter {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            client: OpenAiClient::new(base_url, api_key, model),
        }
    }
}

#[async_trait]
impl Adapter for OpenAiApiAdapter {
    async fn process(&self, request: &PipelineRequest) -> Result<String, AdapterError> {
        let chat_request = ChatCompletionRequest {
            model: self.client.model().to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: request.current_content.clone(),
            }],
            max_tokens: Some(2048),
            temperature: Some(0.7),
        };

        let response = self.client.chat_completion(&chat_request).await?;

        Ok(response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = OpenAiApiAdapter::new(
            "http://localhost:8080".to_string(),
            Some("test-key".to_string()),
            "gpt-4".to_string(),
        );
        // Adapter should be created without panicking
        assert!(std::mem::size_of_val(&adapter) > 0);
    }
}
