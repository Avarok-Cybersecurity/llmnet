//! File fetching for local paths and remote URLs
//!
//! This module provides functionality to fetch files from local filesystem
//! or download from remote URLs with caching support.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors that can occur during file fetching
#[derive(Error, Debug)]
pub enum FetchError {
    #[error("Local file not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Failed to create cache directory: {0}")]
    CacheDirectoryError(String),

    #[error("Download failed: {0}")]
    DownloadError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

/// Classification of a path as local or remote
#[derive(Debug, Clone, PartialEq)]
pub enum PathType {
    /// Local filesystem path
    Local,
    /// Remote URL (http/https)
    Remote,
    /// Huggingface model reference (hf:// or huggingface://)
    Huggingface,
}

// ============================================================================
// SBIO: Pure business logic (no I/O)
// ============================================================================

/// Classify whether a path is local or remote
///
/// Returns `PathType::Remote` for http:// and https:// URLs,
/// `PathType::Huggingface` for hf:// and huggingface:// URLs,
/// and `PathType::Local` for everything else.
pub fn classify_path(path: &str) -> PathType {
    let path_lower = path.to_lowercase();
    if path_lower.starts_with("http://") || path_lower.starts_with("https://") {
        PathType::Remote
    } else if path_lower.starts_with("hf://") || path_lower.starts_with("huggingface://") {
        PathType::Huggingface
    } else {
        PathType::Local
    }
}

/// Generate a deterministic cache path for a remote URL
///
/// Uses SHA256 hash of the URL to create a unique filename,
/// preserving the original file extension if present.
pub fn cache_path_for_url(url: &str, cache_dir: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = hasher.finalize();
    let hash_str = format!("{:x}", hash);

    // Extract extension from URL if present
    let extension = url
        .rsplit('/')
        .next()
        .and_then(|filename| {
            let parts: Vec<&str> = filename.rsplitn(2, '.').collect();
            if parts.len() == 2 {
                Some(parts[0])
            } else {
                None
            }
        })
        .unwrap_or("");

    let filename = if extension.is_empty() {
        hash_str[..16].to_string()
    } else {
        format!("{}.{}", &hash_str[..16], extension)
    };

    cache_dir.join(filename)
}

/// Convert a Huggingface reference to a download URL
///
/// Supports formats:
/// - `hf://org/repo/file.gguf`
/// - `huggingface://org/repo/file.gguf`
pub fn huggingface_to_url(hf_ref: &str) -> Result<String, FetchError> {
    let path = hf_ref
        .strip_prefix("hf://")
        .or_else(|| hf_ref.strip_prefix("huggingface://"))
        .ok_or_else(|| FetchError::InvalidUrl(hf_ref.to_string()))?;

    // Format: org/repo/path/to/file
    let parts: Vec<&str> = path.splitn(3, '/').collect();
    if parts.len() < 3 {
        return Err(FetchError::InvalidUrl(format!(
            "Huggingface reference must be org/repo/file: {}",
            hf_ref
        )));
    }

    Ok(format!(
        "https://huggingface.co/{}/{}/resolve/main/{}",
        parts[0], parts[1], parts[2]
    ))
}

/// Get the default cache directory
pub fn default_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".llmnet")
        .join("cache")
}

// ============================================================================
// I/O boundary functions
// ============================================================================

/// Download a file from a URL to a local path
pub async fn download_file(url: &str, dest: &Path) -> Result<(), FetchError> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| FetchError::DownloadError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(FetchError::DownloadError(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| FetchError::DownloadError(e.to_string()))?;

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| FetchError::CacheDirectoryError(e.to_string()))?;
    }

    std::fs::write(dest, &bytes)?;
    Ok(())
}

/// Fetch a file from a local path or remote URL
///
/// For local paths, returns the path directly after verifying it exists.
/// For remote URLs, downloads to cache and returns the cached path.
/// For Huggingface references, converts to URL and downloads.
pub async fn fetch_file(path: impl AsRef<str>) -> Result<PathBuf, FetchError> {
    let path_str = path.as_ref();

    match classify_path(path_str) {
        PathType::Local => {
            let local_path = PathBuf::from(path_str);
            if local_path.exists() {
                Ok(local_path)
            } else {
                Err(FetchError::FileNotFound(local_path))
            }
        }
        PathType::Remote => {
            let cache_dir = default_cache_dir();
            let cache_path = cache_path_for_url(path_str, &cache_dir);

            // Skip download if already cached
            if !cache_path.exists() {
                download_file(path_str, &cache_path).await?;
            }

            Ok(cache_path)
        }
        PathType::Huggingface => {
            let url = huggingface_to_url(path_str)?;
            let cache_dir = default_cache_dir();
            let cache_path = cache_path_for_url(&url, &cache_dir);

            // Skip download if already cached
            if !cache_path.exists() {
                download_file(&url, &cache_path).await?;
            }

            Ok(cache_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_local_path() {
        assert_eq!(classify_path("/path/to/file"), PathType::Local);
        assert_eq!(classify_path("./relative/path"), PathType::Local);
        assert_eq!(classify_path("file.txt"), PathType::Local);
        assert_eq!(classify_path("tinyllama:1.1b"), PathType::Local);
    }

    #[test]
    fn test_classify_remote_url() {
        assert_eq!(classify_path("http://example.com/file"), PathType::Remote);
        assert_eq!(classify_path("https://example.com/file"), PathType::Remote);
        assert_eq!(classify_path("HTTPS://EXAMPLE.COM/file"), PathType::Remote);
    }

    #[test]
    fn test_classify_huggingface() {
        assert_eq!(
            classify_path("hf://TheBloke/model/file.gguf"),
            PathType::Huggingface
        );
        assert_eq!(
            classify_path("huggingface://org/repo/model.gguf"),
            PathType::Huggingface
        );
    }

    #[test]
    fn test_cache_path_deterministic() {
        let cache_dir = PathBuf::from("/cache");
        let url = "https://example.com/model.gguf";

        let path1 = cache_path_for_url(url, &cache_dir);
        let path2 = cache_path_for_url(url, &cache_dir);

        assert_eq!(path1, path2);
    }

    #[test]
    fn test_cache_path_preserves_extension() {
        let cache_dir = PathBuf::from("/cache");

        let path_gguf = cache_path_for_url("https://example.com/model.gguf", &cache_dir);
        assert!(path_gguf.to_string_lossy().ends_with(".gguf"));

        let path_bin = cache_path_for_url("https://example.com/model.bin", &cache_dir);
        assert!(path_bin.to_string_lossy().ends_with(".bin"));
    }

    #[test]
    fn test_cache_path_different_urls() {
        let cache_dir = PathBuf::from("/cache");

        let path1 = cache_path_for_url("https://example.com/model1.gguf", &cache_dir);
        let path2 = cache_path_for_url("https://example.com/model2.gguf", &cache_dir);

        assert_ne!(path1, path2);
    }

    #[test]
    fn test_huggingface_to_url() {
        let url =
            huggingface_to_url("hf://TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf").unwrap();
        assert_eq!(
            url,
            "https://huggingface.co/TheBloke/Llama-2-7B-GGUF/resolve/main/llama-2-7b.Q4_K_M.gguf"
        );

        let url2 = huggingface_to_url("huggingface://org/repo/file.bin").unwrap();
        assert_eq!(
            url2,
            "https://huggingface.co/org/repo/resolve/main/file.bin"
        );
    }

    #[test]
    fn test_huggingface_to_url_invalid() {
        assert!(huggingface_to_url("hf://org/repo").is_err());
        assert!(huggingface_to_url("not-hf://org/repo/file").is_err());
    }
}
