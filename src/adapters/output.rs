use async_trait::async_trait;

use crate::adapters::openai_api::{Adapter, AdapterError};
use crate::runtime::PipelineRequest;

/// Output adapter - returns the current content without modification
pub struct OutputAdapter;

impl OutputAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OutputAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Adapter for OutputAdapter {
    async fn process(&self, request: &PipelineRequest) -> Result<String, AdapterError> {
        // Simply return the current content
        Ok(request.current_content.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_output_adapter() {
        let adapter = OutputAdapter::new();
        let request = PipelineRequest::new("Test content".to_string());

        let result = adapter.process(&request).await.unwrap();
        assert_eq!(result, "Test content");
    }
}
