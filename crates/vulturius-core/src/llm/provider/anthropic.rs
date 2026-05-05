//! Anthropic (Claude) API provider with retry and rate limiting.

use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{debug, warn};

use super::{parse_analysis_response, prompts, LlmAnalysisResult, ProviderKind};
use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

use crate::llm::rate_limiter::RateLimiter;
use crate::llm::retry::{self, RetryConfig};

/// Anthropic provider with production-grade features.
pub struct AnthropicProvider {
    config: LlmConfig,
    client: Client,
    retry_config: RetryConfig,
    rate_limiter: RateLimiter,
    /// API version header.
    api_version: String,
}

/// Anthropic response structure.
#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new(config: &LlmConfig, rate_limiter: RateLimiter) -> Self {
        Self {
            config: config.clone(),
            client: Client::builder()
                .timeout(Duration::from_secs(config.timeout_seconds))
                .build()
                .unwrap_or_default(),
            retry_config: RetryConfig::default(),
            rate_limiter,
            api_version: "2023-06-01".to_string(),
        }
    }

    /// Analyze a single file using the Anthropic API.
    pub async fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        let prompt = prompts::file_analysis_prompt(entry);
        let response = self.call_api_with_retry(&prompts::FILE_ANALYSIS_SYSTEM, &prompt).await?;

        match parse_analysis_response(&response) {
            Ok(mut results) => Ok(results.pop()),
            Err(e) => {
                warn!("Anthropic: Failed to parse analysis response: {}", e);
                Ok(None)
            }
        }
    }

    /// Analyze a batch of files.
    pub async fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let batch_size = entries.len().min(50);
        let batch_entries: Vec<_> = entries[..batch_size].iter().collect();

        let prompt = prompts::batch_analysis_prompt(&batch_entries);
        let response = self.call_api_with_retry(&prompts::FILE_ANALYSIS_SYSTEM, &prompt).await?;

        parse_analysis_response(&response).map_err(|e| {
            warn!("Anthropic: Batch parse error: {}", e);
            crate::VulturiusError::Llm(format!("Batch parse error: {}", e))
        })
    }

    /// Convert natural language query to scan config.
    pub async fn query_to_scan_config(&self, query: &str) -> Result<String> {
        let prompt = prompts::nl_query_prompt(query);
        self.call_api_with_retry(&prompts::NL_QUERY_SYSTEM, &prompt).await
    }

    /// Call the Anthropic API with retry logic.
    async fn call_api_with_retry(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let provider_name = "anthropic";
        let client = self.client.clone();
        let api_key = self.config.api_key.clone();
        let model = self.config.model.clone();
        let timeout_secs = self.config.timeout_seconds;
        let rate_limiter = self.rate_limiter.clone();
        let retry_config = self.retry_config.clone();
        let api_version = self.api_version.clone();

        // Acquire rate limit slot
        rate_limiter.acquire(provider_name, user_prompt.len() as u64).await;

        let result = retry::retry(&retry_config, || {
            let client = client.clone();
            let api_key = api_key.clone();
            let model = model.clone();
            let api_version = api_version.clone();

            async move {
                let url = "https://api.anthropic.com/v1/messages";

                let response = timeout(
                    Duration::from_secs(timeout_secs),
                    client
                        .post(url)
                        .header("x-api-key", api_key)
                        .header("anthropic-version", &api_version)
                        .header("Content-Type", "application/json")
                        .json(&serde_json::json!({
                            "model": model,
                            "system": system_prompt,
                            "messages": [
                                {"role": "user", "content": user_prompt}
                            ],
                            "temperature": 0.1,
                            "max_tokens": 2000
                        }))
                        .send(),
                )
                .await
                .map_err(|_| crate::VulturiusError::Timeout("Anthropic request timed out".to_string()))?
                .map_err(|e| crate::VulturiusError::Llm(format!("Anthropic request failed: {}", e)))?;

                let status = response.status();

                if retry::is_retryable(status) {
                    let retry_after = retry::retry_after(&response);
                    if let Some(delay) = retry_after {
                        debug!(
                            provider = provider_name,
                            retry_after_secs = delay.as_secs(),
                            "Rate limited, waiting"
                        );
                        tokio::time::sleep(delay).await;
                    }
                    return Err(crate::VulturiusError::Llm(format!(
                        "Retryable error: {}",
                        status
                    )));
                }

                if !status.is_success() {
                    let error_text = response.text().await.unwrap_or_default();
                    return Err(crate::VulturiusError::Llm(format!(
                        "Anthropic API error {}: {}",
                        status, error_text
                    )));
                }

                let json: AnthropicResponse = response
                    .json()
                    .await
                    .map_err(|e| crate::VulturiusError::Llm(format!("Failed to parse response: {}", e)))?;

                // Extract text from content blocks
                let mut content = String::new();
                for block in json.content {
                    if block.block_type == "text" {
                        if let Some(text) = block.text {
                            content.push_str(&text);
                        }
                    }
                }

                if content.is_empty() {
                    return Err(crate::VulturiusError::Llm(
                        "Empty response from Anthropic API".to_string(),
                    ));
                }

                Ok(content)
            }
        })
        .await;

        // Release rate limit slot
        rate_limiter.release(provider_name).await;

        match &result {
            Ok(_) => debug!(provider = provider_name, "Anthropic API call succeeded"),
            Err(e) => warn!(provider = provider_name, error = %e, "Anthropic API call failed"),
        }

        result
    }

    /// Get provider kind.
    pub fn kind(&self) -> ProviderKind {
        ProviderKind::Anthropic
    }

    /// Get provider name for logging.
    pub fn name(&self) -> &str {
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::rate_limiter::RateLimiter;

    fn test_config() -> LlmConfig {
        LlmConfig {
            enabled: true,
            provider: "anthropic".to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            api_key: "test-key".to_string(),
            max_calls_per_scan: 50,
            timeout_seconds: 30,
            cache_decisions: true,
        }
    }

    #[test]
    fn test_anthropic_provider_creation() {
        let config = test_config();
        let limiter = RateLimiter::new();
        let provider = AnthropicProvider::new(&config, limiter);
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.kind(), ProviderKind::Anthropic);
    }
}
