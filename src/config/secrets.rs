//! Secrets loading and resolution
//!
//! This module provides functionality to load secrets from various sources:
//! - Environment files (.env format)
//! - System environment variables
//! - HashiCorp Vault (KV v2 engine)

use std::collections::HashMap;
use std::path::Path;

use dashmap::DashMap;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// SBIO: Pure data structures
// ============================================================================

/// Secret source configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum SecretSource {
    /// Load from a .env file
    EnvFile {
        path: String,
        /// Optional list of variables to load (empty = load all)
        #[serde(default)]
        variables: Vec<String>,
    },
    /// Load from system environment variable
    Env { variable: String },
    /// Load from HashiCorp Vault
    Vault {
        address: String,
        path: String,
        /// Optional list of variables to load (empty = load all)
        #[serde(default)]
        variables: Vec<String>,
        /// Environment variable containing Vault token (default: VAULT_TOKEN)
        #[serde(skip_serializing_if = "Option::is_none")]
        token_env: Option<String>,
    },
}

/// Errors during secret operations
#[derive(Error, Debug)]
pub enum SecretError {
    #[error("Failed to read env file: {0}")]
    EnvFileRead(String),

    #[error("Failed to parse env file: {0}")]
    EnvFileParse(String),

    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),

    #[error("Vault error: {0}")]
    VaultError(String),

    #[error("Secret not found: {0}.{1}")]
    SecretNotFound(String, String),

    #[error("Invalid secret reference: {0}")]
    InvalidReference(String),
}

/// Parsed secret reference
#[derive(Debug, Clone, PartialEq)]
pub struct SecretRef {
    pub secret_name: String,
    pub variable: String,
}

// ============================================================================
// SBIO: Pure functions (no I/O)
// ============================================================================

/// Parse a .env file content into key-value pairs.
/// Handles KEY=VALUE format, # comments, quoted values.
pub fn parse_env_content(content: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Find the first = sign
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim().to_string();
            let mut value = line[pos + 1..].trim().to_string();

            // Remove surrounding quotes if present
            if ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
                && value.len() >= 2
            {
                value = value[1..value.len() - 1].to_string();
            }

            if !key.is_empty() {
                result.insert(key, value);
            }
        }
    }

    result
}

/// Filter a HashMap to only include specified keys.
/// If `filter` is empty, returns all entries.
pub fn filter_variables(
    values: HashMap<String, String>,
    filter: &[String],
) -> HashMap<String, String> {
    if filter.is_empty() {
        values
    } else {
        values
            .into_iter()
            .filter(|(k, _)| filter.contains(k))
            .collect()
    }
}

/// Parse a secret reference like "$secrets.dgx-creds.DGX_API_KEY"
pub fn parse_secret_reference(s: &str) -> Option<SecretRef> {
    let pattern = Regex::new(r"^\$secrets\.([a-zA-Z0-9_-]+)\.([a-zA-Z0-9_]+)$").ok()?;
    let caps = pattern.captures(s)?;

    Some(SecretRef {
        secret_name: caps.get(1)?.as_str().to_string(),
        variable: caps.get(2)?.as_str().to_string(),
    })
}

/// Find all secret references in a string
pub fn find_secret_references(s: &str) -> Vec<SecretRef> {
    let pattern = Regex::new(r"\$secrets\.([a-zA-Z0-9_-]+)\.([a-zA-Z0-9_]+)").unwrap();
    pattern
        .captures_iter(s)
        .filter_map(|caps: regex::Captures| {
            Some(SecretRef {
                secret_name: caps.get(1)?.as_str().to_string(),
                variable: caps.get(2)?.as_str().to_string(),
            })
        })
        .collect()
}

/// Substitute secret references in a string using provided values
pub fn substitute_secrets(template: &str, secrets: &HashMap<String, String>) -> String {
    let pattern = Regex::new(r"\$secrets\.([a-zA-Z0-9_-]+)\.([a-zA-Z0-9_]+)").unwrap();
    pattern
        .replace_all(template, |caps: &regex::Captures<'_>| {
            let key = format!(
                "{}.{}",
                caps.get(1)
                    .map(|m: regex::Match<'_>| m.as_str())
                    .unwrap_or(""),
                caps.get(2)
                    .map(|m: regex::Match<'_>| m.as_str())
                    .unwrap_or("")
            );
            secrets.get(&key).cloned().unwrap_or_default()
        })
        .to_string()
}

/// Substitute secrets in a serde_json::Value recursively
pub fn substitute_secrets_in_value(
    value: &serde_json::Value,
    secrets: &HashMap<String, String>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(substitute_secrets(s, secrets)),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| substitute_secrets_in_value(v, secrets))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| {
                    (
                        substitute_secrets(k, secrets),
                        substitute_secrets_in_value(v, secrets),
                    )
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

// ============================================================================
// SBIO: I/O - SecretsManager
// ============================================================================

/// Manager for loading and resolving secrets
pub struct SecretsManager {
    /// Map of "secret_name.variable" -> value
    secrets: DashMap<String, String>,
}

impl SecretsManager {
    pub fn new() -> Self {
        Self {
            secrets: DashMap::new(),
        }
    }

    /// Load secrets from all configured sources
    pub async fn load_all(
        &self,
        configs: &HashMap<String, SecretSource>,
    ) -> Result<(), SecretError> {
        for (name, source) in configs {
            self.load_source(name, source).await?;
        }
        Ok(())
    }

    /// Load a single secret source
    async fn load_source(&self, name: &str, source: &SecretSource) -> Result<(), SecretError> {
        match source {
            SecretSource::EnvFile { path, variables } => {
                self.load_env_file(name, path, variables).await
            }
            SecretSource::Env { variable } => self.load_env_var(name, variable),
            SecretSource::Vault {
                address,
                path,
                variables,
                token_env,
            } => {
                self.load_vault(name, address, path, variables, token_env.as_deref())
                    .await
            }
        }
    }

    /// Load from .env file
    async fn load_env_file(
        &self,
        name: &str,
        path: &str,
        variables: &[String],
    ) -> Result<(), SecretError> {
        // Expand ~ to home directory
        let expanded = shellexpand::tilde(path);
        let path = Path::new(expanded.as_ref());

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| SecretError::EnvFileRead(format!("{}: {}", path.display(), e)))?;

        let parsed = parse_env_content(&content);
        let filtered = filter_variables(parsed, variables);

        for (key, value) in filtered {
            self.secrets.insert(format!("{}.{}", name, key), value);
        }

        Ok(())
    }

    /// Load from system environment variable
    fn load_env_var(&self, name: &str, variable: &str) -> Result<(), SecretError> {
        let value = std::env::var(variable)
            .map_err(|_| SecretError::EnvVarNotFound(variable.to_string()))?;

        self.secrets.insert(format!("{}.{}", name, variable), value);
        Ok(())
    }

    /// Load from HashiCorp Vault
    async fn load_vault(
        &self,
        name: &str,
        address: &str,
        path: &str,
        variables: &[String],
        token_env: Option<&str>,
    ) -> Result<(), SecretError> {
        let token_var = token_env.unwrap_or("VAULT_TOKEN");
        let token = std::env::var(token_var)
            .map_err(|_| SecretError::VaultError(format!("Token env var {} not set", token_var)))?;

        // Build Vault KV v2 URL
        let url = format!("{}/v1/{}", address.trim_end_matches('/'), path);

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| SecretError::VaultError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(SecretError::VaultError(format!(
                "Vault returned status {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SecretError::VaultError(e.to_string()))?;

        // KV v2 response structure: { "data": { "data": { ... } } }
        let data = body
            .get("data")
            .and_then(|d| d.get("data"))
            .and_then(|d| d.as_object())
            .ok_or_else(|| {
                SecretError::VaultError("Invalid Vault response structure".to_string())
            })?;

        let mut parsed = HashMap::new();
        for (key, value) in data {
            if let Some(s) = value.as_str() {
                parsed.insert(key.clone(), s.to_string());
            }
        }

        let filtered = filter_variables(parsed, variables);
        for (key, value) in filtered {
            self.secrets.insert(format!("{}.{}", name, key), value);
        }

        Ok(())
    }

    /// Resolve a secret by name and variable
    pub fn resolve(&self, secret_name: &str, variable: &str) -> Option<String> {
        let key = format!("{}.{}", secret_name, variable);
        self.secrets.get(&key).map(|v| v.value().clone())
    }

    /// Get all loaded secrets as a HashMap for substitution
    pub fn all_secrets(&self) -> HashMap<String, String> {
        self.secrets
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Substitute secret references in a string
    pub fn substitute(&self, template: &str) -> String {
        substitute_secrets(template, &self.all_secrets())
    }

    /// Substitute secret references in a JSON value
    pub fn substitute_value(&self, value: &serde_json::Value) -> serde_json::Value {
        substitute_secrets_in_value(value, &self.all_secrets())
    }
}

impl Default for SecretsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_content_basic() {
        let content = r#"
KEY1=value1
KEY2=value2
"#;
        let result = parse_env_content(content);
        assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_parse_env_content_with_comments() {
        let content = r#"
# This is a comment
KEY1=value1
# Another comment
KEY2=value2
"#;
        let result = parse_env_content(content);
        assert_eq!(result.len(), 2);
        assert!(!result.contains_key("# This is a comment"));
    }

    #[test]
    fn test_parse_env_content_quoted_values() {
        let content = r#"
DOUBLE="double quoted"
SINGLE='single quoted'
UNQUOTED=no quotes
"#;
        let result = parse_env_content(content);
        assert_eq!(result.get("DOUBLE"), Some(&"double quoted".to_string()));
        assert_eq!(result.get("SINGLE"), Some(&"single quoted".to_string()));
        assert_eq!(result.get("UNQUOTED"), Some(&"no quotes".to_string()));
    }

    #[test]
    fn test_parse_env_content_empty_value() {
        let content = "EMPTY=";
        let result = parse_env_content(content);
        assert_eq!(result.get("EMPTY"), Some(&"".to_string()));
    }

    #[test]
    fn test_filter_variables_empty_filter() {
        let mut values = HashMap::new();
        values.insert("A".to_string(), "1".to_string());
        values.insert("B".to_string(), "2".to_string());

        let filtered = filter_variables(values, &[]);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_variables_with_filter() {
        let mut values = HashMap::new();
        values.insert("A".to_string(), "1".to_string());
        values.insert("B".to_string(), "2".to_string());
        values.insert("C".to_string(), "3".to_string());

        let filtered = filter_variables(values, &["A".to_string(), "C".to_string()]);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.contains_key("A"));
        assert!(filtered.contains_key("C"));
        assert!(!filtered.contains_key("B"));
    }

    #[test]
    fn test_parse_secret_reference_valid() {
        let result = parse_secret_reference("$secrets.dgx-creds.DGX_API_KEY");
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.secret_name, "dgx-creds");
        assert_eq!(r.variable, "DGX_API_KEY");
    }

    #[test]
    fn test_parse_secret_reference_invalid() {
        assert!(parse_secret_reference("not a secret").is_none());
        assert!(parse_secret_reference("$secrets.missing").is_none());
        assert!(parse_secret_reference("$other.name.var").is_none());
    }

    #[test]
    fn test_find_secret_references() {
        let text = "Bearer $secrets.auth.TOKEN and $secrets.db.PASSWORD here";
        let refs = find_secret_references(text);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].secret_name, "auth");
        assert_eq!(refs[0].variable, "TOKEN");
        assert_eq!(refs[1].secret_name, "db");
        assert_eq!(refs[1].variable, "PASSWORD");
    }

    #[test]
    fn test_substitute_secrets() {
        let mut secrets = HashMap::new();
        secrets.insert("auth.TOKEN".to_string(), "secret123".to_string());
        secrets.insert("db.PASSWORD".to_string(), "dbpass".to_string());

        let result = substitute_secrets("Bearer $secrets.auth.TOKEN", &secrets);
        assert_eq!(result, "Bearer secret123");
    }

    #[test]
    fn test_substitute_secrets_in_value() {
        let mut secrets = HashMap::new();
        secrets.insert("auth.KEY".to_string(), "abc123".to_string());

        let value = serde_json::json!({
            "header": "$secrets.auth.KEY",
            "nested": {
                "value": "$secrets.auth.KEY"
            },
            "array": ["$secrets.auth.KEY", "plain"]
        });

        let result = substitute_secrets_in_value(&value, &secrets);
        assert_eq!(result["header"], "abc123");
        assert_eq!(result["nested"]["value"], "abc123");
        assert_eq!(result["array"][0], "abc123");
        assert_eq!(result["array"][1], "plain");
    }

    #[test]
    fn test_secrets_manager_resolve() {
        let manager = SecretsManager::new();
        manager
            .secrets
            .insert("test.VAR".to_string(), "value".to_string());

        assert_eq!(manager.resolve("test", "VAR"), Some("value".to_string()));
        assert_eq!(manager.resolve("test", "MISSING"), None);
    }
}
