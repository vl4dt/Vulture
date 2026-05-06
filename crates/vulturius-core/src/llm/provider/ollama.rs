//! Ollama (local) provider with model auto-discovery.
#![allow(dead_code)]

use std::collections::HashSet;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use super::{parse_analysis_response, prompts, LlmAnalysisResult, ProviderKind};
use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

use crate::llm::rate_limiter::RateLimiter;
use crate::llm::retry::{self, RetryConfig};

/// Ollama provider with model auto-discovery.
pub struct OllamaProvider {
    config: LlmConfig,
    client: Client,
    retry_config: RetryConfig,
    rate_limiter: RateLimiter,
    base_url: String,
    /// Cached available models (populated on first discovery).
    available_models: std::sync::Mutex<Option<HashSet<String>>>,
}

/// Ollama model info.
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaModel {
    /// Full model name with tag.
    pub name: String,
    /// Model size in bytes.
    pub size: u64,
    /// Model digest.
    pub digest: String,
    /// When the model was modified.
    #[serde(rename = "modified_at")]
    pub modified_at: String,
}

/// Ollama /api/tags response.
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    pub fn new(config: &LlmConfig, rate_limiter: RateLimiter) -> Self {
        Self {
            config: config.clone(),
            client: Client::builder()
                .timeout(Duration::from_secs(config.timeout_seconds))
                .build()
                .unwrap_or_default(),
            retry_config: RetryConfig {
                max_retries: 2, // Less retries for local
                base_delay: Duration::from_millis(200),
                max_delay: Duration::from_secs(5),
                backoff_multiplier: 2.0,
                jitter: false,
            },
            rate_limiter,
            base_url: "http://localhost:11434".to_string(),
            available_models: std::sync::Mutex::new(None),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// Discover available models on the Ollama server.
    pub async fn discover_models(&self) -> Result<Vec<OllamaModel>> {
        let url = format!("{}/api/tags", self.base_url);

        let response = timeout(
            Duration::from_secs(10),
            self.client.get(&url).send(),
        )
        .await
        .map_err(|_| crate::VulturiusError::Timeout("Ollama model discovery timed out".to_string()))?
        .map_err(|e| crate::VulturiusError::Llm(format!("Ollama discovery failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::VulturiusError::Llm(format!(
                "Ollama discovery error: {}",
                response.status()
            )));
        }

        let tags: OllamaTagsResponse = response
            .json()
            .await
            .map_err(|e| crate::VulturiusError::Llm(format!("Failed to parse Ollama tags: {}", e)))?;

        // Cache the model names
        let model_names: HashSet<String> = tags.models.iter().map(|m| m.name.clone()).collect();
        let mut cached = self.available_models.lock().unwrap();
        *cached = Some(model_names);

        info!(
            models = tags.models.len(),
            "Ollama: Discovered {} models",
            tags.models.len()
        );

        Ok(tags.models)
    }

    /// Check if a model is available.
    pub async fn is_model_available(&self, model: &str) -> Result<bool> {
        // Try cached models first
        {
            let cached = self.available_models.lock().unwrap();
            if let Some(ref models) = *cached {
                if models.contains(model) {
                    return Ok(true);
                }
            }
        }

        // Discover models if not cached
        let models = self.discover_models().await?;
        Ok(models.iter().any(|m| m.name == model))
    }

    /// Auto-select the best available model.
    /// Priority: llama3, mistral, codellama, any available.
    pub async fn auto_select_model(&self, preferred: &[&str]) -> Result<String> {
        let models = self.discover_models().await?;

        // Try preferred models first
        for pref in preferred {
            if models.iter().any(|m| m.name.starts_with(pref)) {
                info!(model = %pref, "Ollama: Selected preferred model");
                return Ok(pref.to_string());
            }
        }

        // Fall back to first available
        if let Some(first) = models.first() {
            info!(model = %first.name, "Ollama: Selected first available model");
            Ok(first.name.clone())
        } else {
            Err(crate::VulturiusError::Llm(
                "No models available on Ollama server".to_string(),
            ))
        }
    }

    /// Analyze a single file using Ollama.
    pub async fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        let prompt = prompts::file_analysis_prompt(entry);
        let response = self.call_ollama_with_retry(&prompts::FILE_ANALYSIS_SYSTEM, &prompt).await?;

        match parse_analysis_response(&response) {
            Ok(mut results) => Ok(results.pop()),
            Err(e) => {
                warn!("Ollama: Failed to parse analysis response: {}", e);
                Ok(None)
            }
        }
    }

    /// Analyze a batch of files.
    pub async fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = entries.len().min(20); // Smaller batches for local models
        let batch_entries: Vec<_> = entries[..batch_size].iter().collect();

        let prompt = prompts::batch_analysis_prompt(&batch_entries);
        let response = self.call_ollama_with_retry(&prompts::FILE_ANALYSIS_SYSTEM, &prompt).await?;

        parse_analysis_response(&response).map_err(|e| {
            warn!("Ollama: Batch parse error: {}", e);
            crate::VulturiusError::Llm(format!("Batch parse error: {}", e))
        })
    }

    /// Convert natural language query to scan config.
    pub async fn query_to_scan_config(&self, query: &str) -> Result<String> {
        let prompt = prompts::nl_query_prompt(query);
        self.call_ollama_with_retry(&prompts::NL_QUERY_SYSTEM, &prompt).await
    }

    /// Call Ollama API with retry logic.
    async fn call_ollama_with_retry(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let provider_name = "ollama";
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let model = self.config.model.clone();
        let timeout_secs = self.config.timeout_seconds;
        let rate_limiter = self.rate_limiter.clone();
        let retry_config = self.retry_config.clone();

        // Acquire rate limit slot (usually no-op for local)
        rate_limiter.acquire(provider_name, user_prompt.len() as u64).await;

        let result = retry::retry(&retry_config, || {
            let client = client.clone();
            let base_url = base_url.clone();
            let model = model.clone();

            async move {
                let url = format!("{}/api/chat", base_url);

                let response = timeout(
                    Duration::from_secs(timeout_secs),
                    client
                        .post(&url)
                        .json(&serde_json::json!({
                            "model": model,
                            "messages": [
                                {"role": "system", "content": system_prompt},
                                {"role": "user", "content": user_prompt}
                            ],
                            "stream": false
                        }))
                        .send(),
                )
                .await
                .map_err(|_| crate::VulturiusError::Timeout("Ollama request timed out".to_string()))?
                .map_err(|e| crate::VulturiusError::Llm(format!("Ollama request failed: {}", e)))?;

                let status = response.status();

                if retry::is_retryable(status) {
                    return Err(crate::VulturiusError::Llm(format!(
                        "Retryable error: {}",
                        status
                    )));
                }

                if !status.is_success() {
                    let error_text = response.text().await.unwrap_or_default();
                    return Err(crate::VulturiusError::Llm(format!(
                        "Ollama error {}: {}",
                        status, error_text
                    )));
                }

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| crate::VulturiusError::Llm(format!("Failed to parse Ollama response: {}", e)))?;

                json.get("message")
                    .and_then(|msg| msg.get("content"))
                    .and_then(|content| content.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| {
                        crate::VulturiusError::Llm("Invalid response format from Ollama".to_string())
                    })
            }
        })
        .await;

        // Release rate limit slot
        rate_limiter.release(provider_name).await;

        match &result {
            Ok(_) => debug!(provider = provider_name, "Ollama API call succeeded"),
            Err(e) => warn!(provider = provider_name, error = %e, "Ollama API call failed"),
        }

        result
    }

    /// Get provider kind.
    pub fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    /// Get provider name for logging.
    pub fn name(&self) -> &str {
        "ollama"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::rate_limiter::RateLimiter;

    fn test_config() -> LlmConfig {
        LlmConfig {
            enabled: true,
            provider: "ollama".to_string(),
            model: "llama3".to_string(),
            api_key: String::new(), // Ollama doesn't need API key
            max_calls_per_scan: 100,
            timeout_seconds: 60,
            cache_decisions: true,
        }
    }

    #[test]
    fn test_ollama_provider_creation() {
        let config = test_config();
        let limiter = RateLimiter::new();
        let provider = OllamaProvider::new(&config, limiter);
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.kind(), ProviderKind::Ollama);
    }

    #[test]
    fn test_ollama_custom_base_url() {
        let config = test_config();
        let limiter = RateLimiter::new();
        let provider = OllamaProvider::new(&config, limiter)
            .with_base_url("http://custom-host:11434".to_string());
        assert_eq!(provider.base_url, "http://custom-host:11434");
    }
}
