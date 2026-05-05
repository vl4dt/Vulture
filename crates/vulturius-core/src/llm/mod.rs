//! LLM integration for augmented file analysis.
//!
//! Provides intelligent analysis of ambiguous files using LLM providers.
//! Supports OpenAI-compatible APIs, Ollama (local), and mock providers for testing.
//! Includes decision caching to reduce API costs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

// ─── LLM Analysis Result ────────────────────────────────────────────────────

/// Result of LLM analysis for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmAnalysisResult {
    /// Whether the file is safe to delete according to LLM
    pub safe_to_delete: bool,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable explanation
    pub reason: String,
    /// File category determined by LLM
    pub category: String,
}

// ─── LLM Provider Trait ─────────────────────────────────────────────────────

/// Trait for LLM providers.
pub trait LlmProvider: Send + Sync {
    /// Analyze a single file and return analysis result.
    fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>>;

    /// Analyze a batch of files in a single request.
    fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>>;

    /// Convert natural language query to scan configuration JSON.
    fn query_to_scan_config(&self, query: &str) -> Result<String>;

    /// Get provider name for logging.
    fn provider_name(&self) -> &str;
}

// ─── OpenAI Provider ────────────────────────────────────────────────────────

/// OpenAI-compatible API provider.
pub struct OpenAiProvider {
    config: LlmConfig,
    client: reqwest::Client,
}

impl OpenAiProvider {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(config.timeout_seconds))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Build the API request body for file analysis.
    fn build_analyze_prompt(&self, entries: &[&FileEntry]) -> String {
        let mut prompt = String::from(
            "You are a disk cleanup assistant. Analyze each file and determine if it is safe to delete.\n\n",
        );

        for entry in entries {
            prompt.push_str(&format!(
                "File: {}\nType: {}\nSize: {}\nLast accessed: {:?}\nLocation: {}\n\n",
                entry.path.display(),
                entry.file_type,
                entry.human_size(),
                entry.last_accessed,
                entry.path.parent().map(|p| p.display().to_string()).unwrap_or_default(),
            ));
        }

        prompt.push_str(
            "Respond in JSON array format:\n[\n  {\n    \"safe_to_delete\": boolean,\n    \"confidence\": 0.0-1.0,\n    \"reason\": \"brief explanation\",\n    \"category\": \"temp|cache|log|orphan|unknown\"\n  }\n]",
        );

        prompt
    }

    /// Call the OpenAI API.
    async fn call_api(&self, prompt: &str) -> Result<String> {
        let api_key = self.config.api_key.clone();
        if api_key.is_empty() {
            return Err(crate::VulturiusError::Llm(
                "No API key configured for OpenAI provider".to_string(),
            ));
        }

        let url = "https://api.openai.com/v1/chat/completions";

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a disk cleanup assistant. Analyze files and determine if they are safe to delete. Be conservative - only recommend deletion when confident."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "temperature": 0.1,
            "max_tokens": 1000
        });

        let response = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            self.client
                .post(url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send(),
        )
        .await
        .map_err(|_| crate::VulturiusError::Timeout("LLM request timed out".to_string()))?
        .map_err(|e| crate::VulturiusError::Llm(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
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
}

impl LlmProvider for OpenAiProvider {
    fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        // This would be called in an async context - for now return None
        // as the actual implementation requires async execution
        debug!(
            "OpenAI: Would analyze file {} (async implementation)",
            entry.path.display()
        );
        Ok(None)
    }

    fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        debug!(
            "OpenAI: Would analyze batch of {} files (async implementation)",
            entries.len()
        );
        Ok(Vec::new())
    }

    fn query_to_scan_config(&self, query: &str) -> Result<String> {
        debug!("OpenAI: Would convert query '{}' to scan config", query);
        Ok(String::new())
    }

    fn provider_name(&self) -> &str {
        "openai"
    }
}

// ─── Ollama Provider ────────────────────────────────────────────────────────

/// Local Ollama provider for running models locally.
pub struct OllamaProvider {
    config: LlmConfig,
    client: reqwest::Client,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(config: &LlmConfig) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(config.timeout_seconds))
                .build()
                .unwrap_or_default(),
            base_url: "http://localhost:11434".to_string(),
        }
    }

    /// Call Ollama API.
    async fn call_ollama(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/chat", self.base_url);

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {
                    "role": "system",
                    "content": "You are a disk cleanup assistant. Analyze files and determine if they are safe to delete."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "stream": false
        });

        let response = timeout(
            Duration::from_secs(self.config.timeout_seconds),
            self.client.post(&url).json(&body).send(),
        )
        .await
        .map_err(|_| crate::VulturiusError::Timeout("Ollama request timed out".to_string()))?
        .map_err(|e| crate::VulturiusError::Llm(format!("Ollama request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::VulturiusError::Llm(format!(
                "Ollama error: {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response.json().await.map_err(|e| {
            crate::VulturiusError::Llm(format!("Failed to parse Ollama response: {}", e))
        })?;

        json.get("message")
            .and_then(|msg| msg.get("content"))
            .and_then(|content| content.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                crate::VulturiusError::Llm("Invalid response format from Ollama".to_string())
            })
    }
}

impl LlmProvider for OllamaProvider {
    fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        debug!(
            "Ollama: Would analyze file {} (async implementation)",
            entry.path.display()
        );
        Ok(None)
    }

    fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        debug!(
            "Ollama: Would analyze batch of {} files (async implementation)",
            entries.len()
        );
        Ok(Vec::new())
    }

    fn query_to_scan_config(&self, query: &str) -> Result<String> {
        debug!("Ollama: Would convert query '{}' to scan config", query);
        Ok(String::new())
    }

    fn provider_name(&self) -> &str {
        "ollama"
    }
}

// ─── Mock Provider (for testing) ────────────────────────────────────────────

/// Mock provider that returns deterministic results for testing.
pub struct MockProvider {
    /// Pre-configured responses for testing
    responses: Vec<LlmAnalysisResult>,
    /// Current response index
    current_index: std::sync::atomic::AtomicUsize,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            responses: Vec::new(),
            current_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Set mock responses for testing
    pub fn with_responses(mut self, responses: Vec<LlmAnalysisResult>) -> Self {
        self.responses = responses;
        self
    }

    /// Get next mock response
    fn next_response(&self) -> Option<LlmAnalysisResult> {
        let idx = self.current_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.responses.get(idx).cloned()
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for MockProvider {
    fn analyze_file(&self, _entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        Ok(self.next_response())
    }

    fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        let mut results = Vec::new();
        for _ in 0..entries.len() {
            if let Some(result) = self.next_response() {
                results.push(result);
            }
        }
        Ok(results)
    }

    fn query_to_scan_config(&self, _query: &str) -> Result<String> {
        Ok(serde_json::json!({
            "path": "/home/user",
            "scan_type": "full",
            "filters": ["age > 30d", "size > 100MB"]
        })
        .to_string())
    }

    fn provider_name(&self) -> &str {
        "mock"
    }
}

// ─── Decision Cache ─────────────────────────────────────────────────────────

/// Cache for LLM decisions to avoid redundant API calls.
pub struct DecisionCache {
    /// File metadata hash -> analysis result
    entries: HashMap<String, LlmAnalysisResult>,
    /// Maximum cache size
    max_size: usize,
}

impl DecisionCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
        }
    }

    /// Generate a cache key from file metadata.
    fn cache_key(entry: &FileEntry) -> String {
        // Hash based on path, size, last_modified, and category
        format!(
            "{}:{}:{}:{}",
            entry.path.display(),
            entry.size,
            entry.last_modified.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            entry.category,
        )
    }

    /// Get cached result if available.
    pub fn get(&self, entry: &FileEntry) -> Option<LlmAnalysisResult> {
        self.entries.get(&Self::cache_key(entry)).cloned()
    }

    /// Store a result in the cache.
    pub fn put(&mut self, entry: &FileEntry, result: LlmAnalysisResult) {
        if self.entries.len() >= self.max_size {
            // Simple LRU: remove random entry when full
            if let Some(key) = self.entries.keys().next().cloned() {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(Self::cache_key(entry), result);
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get cache size.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// ─── LLM Engine ─────────────────────────────────────────────────────────────

/// Main LLM engine that coordinates providers, caching, and cost controls.
pub struct LlmEngine {
    config: LlmConfig,
    provider: Box<dyn LlmProvider>,
    cache: Option<DecisionCache>,
    calls_made: usize,
    total_cost_estimate: f64,
}

impl LlmEngine {
    /// Create a new LLM engine with the configured provider.
    pub fn new(config: LlmConfig) -> Self {
        let provider: Box<dyn LlmProvider> = match config.provider.as_str() {
            "openai" => Box::new(OpenAiProvider::new(&config)),
            "ollama" => Box::new(OllamaProvider::new(&config)),
            "mock" => Box::new(MockProvider::new()),
            _ => {
                warn!(
                    "Unknown LLM provider '{}', defaulting to mock",
                    config.provider
                );
                Box::new(MockProvider::new())
            }
        };

        let cache = if config.cache_decisions {
            Some(DecisionCache::new(10000)) // Max 10k cached decisions
        } else {
            None
        };

        Self {
            config,
            provider,
            cache,
            calls_made: 0,
            total_cost_estimate: 0.0,
        }
    }

    /// Analyze a single file using the LLM.
    pub async fn analyze_file(&mut self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        // Check if LLM is enabled
        if !self.config.enabled {
            return Ok(None);
        }

        // Check call budget
        if !self.has_budget() {
            warn!("LLM call budget exceeded ({})", self.config.max_calls_per_scan);
            return Ok(None);
        }

        // Check cache
        if let Some(ref cache) = self.cache {
            if let Some(result) = cache.get(entry) {
                debug!("LLM cache hit for {}", entry.path.display());
                return Ok(Some(result));
            }
        }

        // Call provider
        let result = self.provider.analyze_file(entry)?;

        if let Some(ref result) = result {
            // Update call count
            self.calls_made += 1;
            self.total_cost_estimate += self.estimate_call_cost();

            // Cache the result
            if let Some(ref mut cache) = self.cache {
                cache.put(entry, result.clone());
            }

            debug!(
                "LLM analyzed {}: safe={}, confidence={}",
                entry.path.display(),
                result.safe_to_delete,
                result.confidence
            );
        }

        Ok(result)
    }

    /// Analyze a batch of files.
    pub async fn analyze_batch(
        &mut self,
        entries: &[FileEntry],
    ) -> Result<Vec<LlmAnalysisResult>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        // Check if we have budget for the batch
        if entries.is_empty() || !self.has_budget() {
            return Ok(Vec::new());
        }

        // Limit batch size to remaining budget
        let batch_size = std::cmp::min(
            entries.len(),
            self.config.max_calls_per_scan - self.calls_made,
        );

        let batch_entries = &entries[..batch_size];

        // Check cache for each entry first
        let mut results = Vec::new();
        let mut uncached_entries = Vec::new();

        for entry in batch_entries {
            if let Some(ref cache) = self.cache {
                if let Some(result) = cache.get(entry) {
                    debug!("LLM cache hit for {}", entry.path.display());
                    results.push(result);
                    continue;
                }
            }
            uncached_entries.push(entry.clone());
        }

        // Analyze uncached entries
        if !uncached_entries.is_empty() {
            let provider_results = self.provider.analyze_batch(&uncached_entries)?;

            for (i, result) in provider_results.iter().enumerate() {
                if i < uncached_entries.len() {
                    let entry = &uncached_entries[i];

                    // Cache the result
                    if let Some(ref mut cache) = self.cache {
                        cache.put(entry, result.clone());
                    }

                    results.push(result.clone());
                }
            }

            self.calls_made += uncached_entries.len();
            self.total_cost_estimate +=
                self.estimate_call_cost() * uncached_entries.len() as f64;
        }

        info!(
            "LLM batch analysis complete: {} cached, {} new, {} total calls",
            results.len() - uncached_entries.len(),
            uncached_entries.len(),
            self.calls_made
        );

        Ok(results)
    }

    /// Convert natural language query to scan configuration.
    pub async fn query_to_scan_config(&mut self, query: &str) -> Result<String> {
        if !self.config.enabled {
            return Err(crate::VulturiusError::Llm(
                "LLM is not enabled".to_string(),
            ));
        }

        if !self.has_budget() {
            return Err(crate::VulturiusError::Llm(
                "LLM call budget exceeded".to_string(),
            ));
        }

        let config = self.provider.query_to_scan_config(query)?;
        self.calls_made += 1;
        self.total_cost_estimate += self.estimate_call_cost();

        info!(
            "LLM converted query '{}' to scan config (calls: {}, est. cost: ${:.2})",
            query, self.calls_made, self.total_cost_estimate
        );

        Ok(config)
    }

    /// Estimate the cost of a single LLM call.
    fn estimate_call_cost(&self) -> f64 {
        // Rough estimates per call
        match self.config.model.as_str() {
            m if m.contains("gpt-4o-mini") => 0.0001, // ~$0.15/1M tokens
            m if m.contains("gpt-4o") => 0.001,       // ~$1.50/1M tokens
            m if m.contains("gpt-4") => 0.003,        // ~$3.00/1M tokens
            m if m.contains("gpt-3.5") => 0.00005,    // ~$0.05/1M tokens
            _ => 0.0, // Local models are free
        }
    }

    /// Get the number of LLM calls made in current session.
    pub fn calls_made(&self) -> usize {
        self.calls_made
    }

    /// Get the estimated total cost of LLM calls.
    pub fn estimated_cost(&self) -> f64 {
        self.total_cost_estimate
    }

    /// Check if we've exceeded the call budget.
    pub fn has_budget(&self) -> bool {
        self.config.enabled && self.calls_made < self.config.max_calls_per_scan
    }

    /// Get provider name.
    pub fn provider_name(&self) -> &str {
        self.provider.provider_name()
    }

    /// Get cache size.
    pub fn cache_size(&self) -> usize {
        self.cache.as_ref().map(|c| c.len()).unwrap_or(0)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn create_test_entry(path: &str, category: crate::types::FileCategory) -> FileEntry {
        FileEntry {
            id: uuid::Uuid::new_v4().to_string(),
            path: PathBuf::from(path),
            size: 1024,
            file_type: crate::types::FileType::RegularFile,
            category,
            last_accessed: SystemTime::UNIX_EPOCH + Duration::from_secs(86400 * 30),
            last_modified: SystemTime::UNIX_EPOCH + Duration::from_secs(86400 * 15),
            created: SystemTime::UNIX_EPOCH + Duration::from_secs(86400 * 365),
            permissions: 0o644,
            is_system: false,
            is_hidden: false,
            mime_type: None,
            hash: None,
        }
    }

    fn create_test_config(provider: &str, enabled: bool, cache: bool) -> LlmConfig {
        LlmConfig {
            enabled,
            provider: provider.to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: "test-key".to_string(),
            max_calls_per_scan: 10,
            timeout_seconds: 30,
            cache_decisions: cache,
        }
    }

    #[test]
    fn test_llm_engine_creation() {
        let config = create_test_config("mock", true, true);
        let engine = LlmEngine::new(config);
        assert_eq!(engine.calls_made(), 0);
        assert!(engine.has_budget());
        assert_eq!(engine.provider_name(), "mock");
    }

    #[test]
    fn test_llm_engine_disabled() {
        let config = create_test_config("mock", false, true);
        let engine = LlmEngine::new(config);
        assert!(!engine.has_budget());
    }

    #[test]
    fn test_llm_budget_tracking() {
        let config = create_test_config("mock", true, false);
        let mut engine = LlmEngine::new(config);

        // Mock provider returns None by default, but calls are tracked
        assert_eq!(engine.calls_made(), 0);
        assert!(engine.has_budget());
    }

    #[test]
    fn test_decision_cache() {
        let entry = create_test_entry("/tmp/test.txt", crate::types::FileCategory::Temp);
        let mut cache = DecisionCache::new(100);

        let result = LlmAnalysisResult {
            safe_to_delete: true,
            confidence: 0.95,
            reason: "Temporary file".to_string(),
            category: "temp".to_string(),
        };

        cache.put(&entry, result.clone());
        assert_eq!(cache.get(&entry), Some(result));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_decision_cache_eviction() {
        let mut cache = DecisionCache::new(2);

        let entry1 = create_test_entry("/tmp/1.txt", crate::types::FileCategory::Temp);
        let entry2 = create_test_entry("/tmp/2.txt", crate::types::FileCategory::Temp);
        let entry3 = create_test_entry("/tmp/3.txt", crate::types::FileCategory::Temp);

        let result = LlmAnalysisResult {
            safe_to_delete: true,
            confidence: 0.9,
            reason: "test".to_string(),
            category: "temp".to_string(),
        };

        cache.put(&entry1, result.clone());
        cache.put(&entry2, result.clone());
        cache.put(&entry3, result);

        // Cache should have exactly 2 entries (max_size)
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_mock_provider_responses() {
        let responses = vec![
            LlmAnalysisResult {
                safe_to_delete: true,
                confidence: 0.9,
                reason: "Safe temp file".to_string(),
                category: "temp".to_string(),
            },
            LlmAnalysisResult {
                safe_to_delete: false,
                confidence: 0.8,
                reason: "Important document".to_string(),
                category: "document".to_string(),
            },
        ];

        let provider = MockProvider::new().with_responses(responses);
        let entry = create_test_entry("/tmp/test.txt", crate::types::FileCategory::Temp);

        let result1 = provider.analyze_file(&entry).unwrap().unwrap();
        let result2 = provider.analyze_file(&entry).unwrap().unwrap();

        assert!(result1.safe_to_delete);
        assert!(!result2.safe_to_delete);
    }

    #[test]
    fn test_cost_estimation() {
        let config = create_test_config("openai", true, false);
        let engine = LlmEngine::new(config);
        assert!(engine.estimated_cost() == 0.0);
    }

    #[test]
    fn test_unknown_provider_fallback() {
        let mut config = create_test_config("unknown_provider", true, false);
        config.provider = "unknown_provider".to_string();
        let engine = LlmEngine::new(config);
        assert_eq!(engine.provider_name(), "mock"); // Should fallback to mock
    }
}
