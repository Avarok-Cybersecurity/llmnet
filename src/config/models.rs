use serde::{Deserialize, Serialize};

/// Model definition types that can be specified in the composition file.
/// Using tagged union for cleaner JSON representation.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ModelDefinition {
    External(ExternalModel),
    Docker(DockerModel),
    Huggingface(HuggingfaceModel),
}

/// External OpenAI-compatible API endpoint
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ExternalModel {
    pub interface: String,
    pub url: String,
    #[serde(rename = "api-key")]
    pub api_key: Option<String>,
}

/// Docker-based model runner
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DockerModel {
    pub image: String,
    pub pat: Option<String>,
    pub registry_url: Option<String>,
    pub params: Option<String>,
}

/// HuggingFace model with runner specification
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HuggingfaceModel {
    pub url: String,
    pub hf_pat: Option<String>,
    pub runner: String,
}

impl ModelDefinition {
    /// Get the model type as a string for display
    pub fn type_name(&self) -> &'static str {
        match self {
            ModelDefinition::External(_) => "external",
            ModelDefinition::Docker(_) => "docker",
            ModelDefinition::Huggingface(_) => "huggingface",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_external_model() {
        let json = r#"{
            "type": "external",
            "interface": "openai-api",
            "url": "https://api.example.com",
            "api-key": "sk-test"
        }"#;

        let model: ModelDefinition = serde_json::from_str(json).unwrap();
        match model {
            ModelDefinition::External(ext) => {
                assert_eq!(ext.interface, "openai-api");
                assert_eq!(ext.url, "https://api.example.com");
                assert_eq!(ext.api_key, Some("sk-test".to_string()));
            }
            _ => panic!("Expected External model"),
        }
    }

    #[test]
    fn test_parse_huggingface_model() {
        let json = r#"{
            "type": "huggingface",
            "url": "cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit",
            "runner": "ollama"
        }"#;

        let model: ModelDefinition = serde_json::from_str(json).unwrap();
        match model {
            ModelDefinition::Huggingface(hf) => {
                assert_eq!(hf.url, "cyankiwi/Nemotron-Orchestrator-8B-AWQ-4bit");
                assert_eq!(hf.runner, "ollama");
                assert_eq!(hf.hf_pat, None);
            }
            _ => panic!("Expected Huggingface model"),
        }
    }

    #[test]
    fn test_parse_docker_model() {
        let json = r#"{
            "type": "docker",
            "image": "some-image-name",
            "params": "vllm serve --model foo"
        }"#;

        let model: ModelDefinition = serde_json::from_str(json).unwrap();
        match model {
            ModelDefinition::Docker(docker) => {
                assert_eq!(docker.image, "some-image-name");
                assert_eq!(docker.params, Some("vllm serve --model foo".to_string()));
            }
            _ => panic!("Expected Docker model"),
        }
    }

    #[test]
    fn test_type_name() {
        let ext = ModelDefinition::External(ExternalModel {
            interface: "openai-api".to_string(),
            url: "http://test".to_string(),
            api_key: None,
        });
        assert_eq!(ext.type_name(), "external");
    }
}
