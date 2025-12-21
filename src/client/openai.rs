use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Data structures (pure, no I/O)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("JSON parse error: {0}")]
    Parse(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
}

// ============================================================================
// SBIO: Trait for abstraction (allows mocking in tests)
// ============================================================================

#[async_trait]
pub trait OpenAiClientTrait: Send + Sync {
    async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ClientError>;
}

// ============================================================================
// SBIO: I/O implementation (real HTTP client)
// ============================================================================

#[derive(Clone)]
pub struct OpenAiClient {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl OpenAiClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
            model,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl OpenAiClientTrait for OpenAiClient {
    async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ClientError> {
        let url = format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'));

        let mut req = self.client.post(&url).json(request);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await.map_err(|e| ClientError::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ClientError::Api {
                status: status.as_u16(),
                message: text,
            });
        }

        let response: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| ClientError::Parse(e.to_string()))?;

        Ok(response)
    }
}

// ============================================================================
// SBIO: Mock implementation for testing (no I/O)
// ============================================================================

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    pub struct MockOpenAiClient {
        responses: Vec<String>,
        call_count: Arc<AtomicUsize>,
    }

    impl MockOpenAiClient {
        pub fn new(responses: Vec<String>) -> Self {
            Self {
                responses,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        pub fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl OpenAiClientTrait for MockOpenAiClient {
        async fn chat_completion(
            &self,
            _request: &ChatCompletionRequest,
        ) -> Result<ChatCompletionResponse, ClientError> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            let content = self
                .responses
                .get(idx)
                .cloned()
                .unwrap_or_else(|| "default response".to_string());

            Ok(ChatCompletionResponse {
                id: format!("mock-{}", idx),
                choices: vec![Choice {
                    index: 0,
                    message: Message {
                        role: "assistant".to_string(),
                        content,
                    },
                    finish_reason: Some("stop".to_string()),
                }],
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_request_serialization() {
        let req = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "test".to_string(),
            }],
            max_tokens: Some(100),
            temperature: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("max_tokens"));
        assert!(!json.contains("temperature")); // None should be skipped
    }

    #[test]
    fn test_response_deserialization() {
        let json = r#"{
            "id": "chatcmpl-123",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello!"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }"#;

        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, "chatcmpl-123");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, "Hello!");
    }

    #[tokio::test]
    async fn test_mock_client() {
        let client = mock::MockOpenAiClient::new(vec![
            "response1".to_string(),
            "response2".to_string(),
        ]);

        let req = ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![],
            max_tokens: None,
            temperature: None,
        };

        let resp1 = client.chat_completion(&req).await.unwrap();
        assert_eq!(resp1.choices[0].message.content, "response1");

        let resp2 = client.chat_completion(&req).await.unwrap();
        assert_eq!(resp2.choices[0].message.content, "response2");

        assert_eq!(client.call_count(), 2);
    }
}
