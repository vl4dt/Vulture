//! OpenAI-compatible API provider with retry, rate limiting, and streaming.
#![allow(dead_code)]

use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;
use tokio::time::timeout;
use tracing::{debug, warn};

use super::{parse_analysis_response, prompts, LlmAnalysisResult, ProviderKind, StreamChunk};
use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

use crate::llm::rate_limiter::RateLimiter;
use crate::llm::retry::{self, RetryConfig};

/// OpenAI provider with production-grade features.
pub struct OpenAiProvider {
    config: LlmConfig,
    client: Client,
    retry_config: RetryConfig,
    rate_limiter: RateLimiter,
    /// Base URL (overridable for OpenAI-compatible APIs).
    base_url: String,
}

/// OpenAI response structure.
#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiMessage>,
    delta: Option<OpenAiDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider.
    pub fn new(config: &LlmConfig, rate_limiter: RateLimiter) -> Self {
        Self {
            config: config.clone(),
            client: Client::builder()
                .timeout(Duration::from_secs(config.timeout_seconds))
                .build()
                .unwrap_or_default(),
            retry_config: RetryConfig::default(),
            rate_limiter,
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }

    /// Set a custom base URL (for OpenAI-compatible APIs).
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// Analyze a single file using the OpenAI API.
    pub async fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        let prompt = prompts::file_analysis_prompt(entry);
        let response = self.call_api_with_retry(&prompts::FILE_ANALYSIS_SYSTEM, &prompt).await?;

        match parse_analysis_response(&response) {
            Ok(mut results) => Ok(results.pop()),
            Err(e) => {
                warn!("OpenAI: Failed to parse analysis response: {}", e);
                Ok(None)
            }
        }
    }

    /// Analyze a batch of files.
    pub async fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Limit batch size to avoid huge prompts
        let batch_size = entries.len().min(50);
        let batch_entries: Vec<_> = entries[..batch_size].iter().collect();

        let prompt = prompts::batch_analysis_prompt(&batch_entries);
        let response = self.call_api_with_retry(&prompts::FILE_ANALYSIS_SYSTEM, &prompt).await?;

        parse_analysis_response(&response).map_err(|e| {
            warn!("OpenAI: Batch parse error: {}", e);
            crate::VulturiusError::Llm(format!("Batch parse error: {}", e))
        })
    }

    /// Analyze files with streaming response.
    /// Returns a channel receiver for stream chunks.
    pub async fn analyze_stream(
        &self,
        entries: &[FileEntry],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk>>> {
        if entries.is_empty() {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let _ = tx.send(Ok(StreamChunk::Done)).await;
            return Ok(rx);
        }

        let batch_size = entries.len().min(50);
        let batch_entries: Vec<_> = entries[..batch_size].to_vec();
        let prompt = prompts::batch_analysis_prompt(&batch_entries.iter().collect::<Vec<_>>());
        let system_prompt = prompts::FILE_ANALYSIS_SYSTEM.to_string();

        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let api_key = self.config.api_key.clone();
        let model = self.config.model.clone();
        let timeout_secs = self.config.timeout_seconds;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            // Send progress
            let _ = tx.send(Ok(StreamChunk::Progress {
                current: 0,
                total: batch_entries.len(),
            })).await;

            // Make streaming request
            let url = format!("{}/chat/completions", base_url);
            let response = match timeout(
                Duration::from_secs(timeout_secs),
                client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": model,
                        "messages": [
                            {"role": "system", "content": &system_prompt},
                            {"role": "user", "content": prompt}
                        ],
                        "temperature": 0.1,
                        "max_tokens": 2000,
                        "stream": true
                    }))
                    .send(),
            ).await {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => {
                    let _ = tx.send(Err(crate::VulturiusError::Llm(format!("Stream request failed: {}", e))))
                        .await;
                    return;
                }
                Err(_) => {
                    let _ = tx.send(Err(crate::VulturiusError::Timeout("LLM stream timed out".to_string())))
                        .await;
                    return;
                }
            };

            if !response.status().is_success() {
                let _ = tx.send(Err(crate::VulturiusError::Llm(format!("API error: {}", response.status()))))
                    .await;
                return;
            }

            // Accumulate response text
            let mut full_response = String::new();
            let mut chunks_processed = 0;

            let mut byte_stream = response.bytes_stream();
            use futures::StreamExt;

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx.send(Err(crate::VulturiusError::Llm(format!("Stream error: {}", e))))
                            .await;
                        return;
                    }
                };
                let text = String::from_utf8_lossy(&bytes);

                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            break;
                        }

                        // Parse SSE chunk
                        if let Ok(parsed) = serde_json::from_str::<OpenAiResponse>(data) {
                            if let Some(choice) = parsed.choices.first() {
                                if let Some(content) = &choice.delta.as_ref().and_then(|d| d.content.clone()) {
                                    full_response.push_str(content);
                                    chunks_processed += 1;

                                    let _ = tx.send(Ok(StreamChunk::Text(content.clone()))).await;
                                    let _ = tx.send(Ok(StreamChunk::Progress {
                                        current: chunks_processed,
                                        total: batch_entries.len(),
                                    })).await;
                                }
                            }
                        }
                    }
                }
            }

            // Parse final results
            match parse_analysis_response(&full_response) {
                Ok(results) => {
                    for result in results {
                        let _ = tx.send(Ok(StreamChunk::Analysis(result))).await;
                    }
                }
                Err(e) => {
                    warn!("OpenAI stream: Failed to parse final response: {}", e);
                    let _ = tx.send(Err(crate::VulturiusError::Llm(format!("Parse error: {}", e)))).await;
                }
            }

            let _ = tx.send(Ok(StreamChunk::Done)).await;
        });

        Ok(rx)
    }

    /// Convert natural language query to scan config.
    pub async fn query_to_scan_config(&self, query: &str) -> Result<String> {
        let prompt = prompts::nl_query_prompt(query);
        self.call_api_with_retry(&prompts::NL_QUERY_SYSTEM, &prompt).await
    }

    /// Call the OpenAI API with retry logic.
    async fn call_api_with_retry(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        let provider_name = "openai";
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let api_key = self.config.api_key.clone();
        let model = self.config.model.clone();
        let timeout_secs = self.config.timeout_seconds;
        let rate_limiter = self.rate_limiter.clone();
        let retry_config = self.retry_config.clone();

        // Acquire rate limit slot
        rate_limiter.acquire(provider_name, user_prompt.len() as u64).await;

        let result = retry::retry(&retry_config, || {
            let client = client.clone();
            let base_url = base_url.clone();
            let api_key = api_key.clone();
            let model = model.clone();

            async move {
                let url = format!("{}/chat/completions", base_url);

                let response = timeout(
                    Duration::from_secs(timeout_secs),
                    client
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", api_key))
                        .header("Content-Type", "application/json")
                        .json(&serde_json::json!({
                            "model": model,
                            "messages": [
                                {"role": "system", "content": system_prompt},
                                {"role": "user", "content": user_prompt}
                            ],
                            "temperature": 0.1,
                            "max_tokens": 2000
                        }))
                        .send(),
                )
                .await
                .map_err(|_| crate::VulturiusError::Timeout("LLM request timed out".to_string()))?
                .map_err(|e| crate::VulturiusError::Llm(format!("API request failed: {}", e)))?;

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
                        "API error {}: {}",
                        status, error_text
                    )));
                }

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| crate::VulturiusError::Llm(format!("Failed to parse response: {}", e)))?;

                json.get("choices")
                    .and_then(|choices| choices.get(0))
                    .and_then(|choice| choice.get("message"))
                    .and_then(|msg| msg.get("content"))
                    .and_then(|content| content.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| {
                        crate::VulturiusError::Llm("Invalid response format from API".to_string())
                    })
            }
        })
        .await;

        // Release rate limit slot
        rate_limiter.release(provider_name).await;

        match &result {
            Ok(_) => debug!(provider = provider_name, "OpenAI API call succeeded"),
            Err(e) => warn!(provider = provider_name, error = %e, "OpenAI API call failed"),
        }

        result
    }

    /// Get provider kind.
    pub fn kind(&self) -> ProviderKind {
        ProviderKind::OpenAi
    }

    /// Get provider name for logging.
    pub fn name(&self) -> &str {
        "openai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::rate_limiter::RateLimiter;

    fn test_config() -> LlmConfig {
        LlmConfig {
            enabled: true,
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: "test-key".to_string(),
            max_calls_per_scan: 50,
            timeout_seconds: 30,
            cache_decisions: true,
        }
    }

    #[test]
    fn test_openai_provider_creation() {
        let config = test_config();
        let limiter = RateLimiter::new();
        let provider = OpenAiProvider::new(&config, limiter);
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.kind(), ProviderKind::OpenAi);
    }

    #[test]
    fn test_openai_custom_base_url() {
        let config = test_config();
        let limiter = RateLimiter::new();
        let provider = OpenAiProvider::new(&config, limiter)
            .with_base_url("http://localhost:8080/v1".to_string());
        assert_eq!(provider.base_url, "http://localhost:8080/v1");
    }
}
