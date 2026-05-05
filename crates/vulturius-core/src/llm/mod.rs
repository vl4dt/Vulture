//! LLM integration for augmented file analysis.
//!
//! Provides intelligent analysis of ambiguous files using LLM providers.
//! Supports OpenAI, Anthropic, Ollama (local), and mock providers for testing.
//! Includes decision caching, rate limiting, retry with exponential backoff,
//! provider fallback chains, streaming responses, and encrypted API key storage.

mod fallback;
mod key_store;
mod provider;
mod rate_limiter;
mod retry;
mod streaming;

pub use fallback::FallbackChain;
pub use key_store::KeyStore;
pub use provider::{
    parse_analysis_response, prompts, LlmAnalysisResult, ProviderKind, StreamChunk,
};
pub use rate_limiter::{RateLimiter, RateLimitConfig};
pub use retry::RetryConfig;
pub use streaming::{AnalysisProgress, StreamingAnalysis, run_streaming};

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

// ─── Decision Cache ─────────────────────────────────────────────────────────

/// Cache for LLM decisions to avoid redundant API calls.
pub struct DecisionCache {
    /// File metadata hash -> analysis result.
    entries: HashMap<String, LlmAnalysisResult>,
    /// Maximum cache size.
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
        format!(
            "{}:{}:{}:{}",
            entry.path.display(),
            entry.size,
            entry
                .last_modified
                .duration_since(std::time::UNIX_EPOCH)
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

/// Main LLM engine that coordinates providers, caching, rate limiting, and cost controls.
pub struct LlmEngine {
    /// Primary config.
    config: LlmConfig,
    /// Fallback chain of providers.
    chain: FallbackChain,
    /// Decision cache.
    cache: Option<DecisionCache>,
    /// Rate limiter.
    rate_limiter: RateLimiter,
    /// Calls made in current session.
    calls_made: usize,
    /// Total estimated cost.
    total_cost_estimate: f64,
    /// API key store for encrypted storage.
    key_store: Option<KeyStore>,
}

impl LlmEngine {
    /// Create a new LLM engine with the configured provider.
    pub fn new(config: LlmConfig) -> Self {
        let rate_limiter = RateLimiter::new();

        // Set provider-specific rate limits
        match config.provider.as_str() {
            "openai" => rate_limiter.set_config_sync("openai", RateLimitConfig::openai_default()),
            "anthropic" => rate_limiter.set_config_sync("anthropic", RateLimitConfig::anthropic_default()),
            _ => rate_limiter.set_config_sync(&config.provider, RateLimitConfig::unlimited()),
        }

        let chain = FallbackChain::single(&config, rate_limiter.clone());

        let cache = if config.cache_decisions {
            Some(DecisionCache::new(10_000))
        } else {
            None
        };

        Self {
            config,
            chain,
            cache,
            rate_limiter,
            calls_made: 0,
            total_cost_estimate: 0.0,
            key_store: None,
        }
    }

    /// Create with a fallback chain (multiple providers).
    pub fn with_fallback(configs: Vec<LlmConfig>) -> Self {
        let primary = configs.first().cloned().unwrap_or_else(|| LlmConfig {
            enabled: false,
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: String::new(),
            max_calls_per_scan: 50,
            timeout_seconds: 30,
            cache_decisions: true,
        });
        let rate_limiter = RateLimiter::new();

        // Configure rate limits for all providers
        for cfg in &configs {
            match cfg.provider.as_str() {
                "openai" => rate_limiter.set_config_sync("openai", RateLimitConfig::openai_default()),
                "anthropic" => rate_limiter.set_config_sync("anthropic", RateLimitConfig::anthropic_default()),
                _ => rate_limiter.set_config_sync(&cfg.provider, RateLimitConfig::unlimited()),
            }
        }

        let chain = FallbackChain::from_configs(&configs, rate_limiter.clone());
        let cache = if primary.cache_decisions {
            Some(DecisionCache::new(10_000))
        } else {
            None
        };

        Self {
            config: primary,
            chain,
            cache,
            rate_limiter,
            calls_made: 0,
            total_cost_estimate: 0.0,
            key_store: None,
        }
    }

    /// Set the API key store for encrypted key management.
    pub fn with_key_store(mut self, store_path: PathBuf) -> Self {
        self.key_store = Some(KeyStore::new(store_path));
        self
    }

    /// Store an API key encrypted.
    pub fn store_api_key(&self, name: &str, value: &str) -> Result<()> {
        if let Some(ref store) = self.key_store {
            store.store_key(name, value)?;
        }
        Ok(())
    }

    /// Retrieve an API key from encrypted storage.
    pub fn get_api_key(&self, name: &str) -> Result<Option<String>> {
        if let Some(ref store) = self.key_store {
            store.get_key(name)
        } else {
            Ok(None)
        }
    }

    /// Analyze a single file using the LLM.
    pub async fn analyze_file(&mut self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        if !self.config.enabled {
            return Ok(None);
        }

        if !self.has_budget() {
            warn!(
                "LLM call budget exceeded ({})",
                self.config.max_calls_per_scan
            );
            return Ok(None);
        }

        // Check cache
        if let Some(ref cache) = self.cache {
            if let Some(result) = cache.get(entry) {
                debug!(
                    "LLM cache hit for {}",
                    entry.path.display()
                );
                return Ok(Some(result));
            }
        }

        // Call provider chain
        let result = self.chain.analyze_file(entry).await?;

        if let Some(ref result) = result {
            self.calls_made += 1;
            self.total_cost_estimate += self.estimate_call_cost();

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

        if entries.is_empty() || !self.has_budget() {
            return Ok(Vec::new());
        }

        // Check cache for each entry first
        let mut results = Vec::new();
        let mut uncached_entries = Vec::new();

        for entry in entries {
            if let Some(ref cache) = self.cache {
                if let Some(result) = cache.get(entry) {
                    debug!(
                        "LLM cache hit for {}",
                        entry.path.display()
                    );
                    results.push(result);
                    continue;
                }
            }
            uncached_entries.push(entry.clone());
        }

        // Analyze uncached entries
        if !uncached_entries.is_empty() {
            let provider_results = self.chain.analyze_batch(&uncached_entries).await?;

            for (i, result) in provider_results.iter().enumerate() {
                if i < uncached_entries.len() {
                    let entry = &uncached_entries[i];

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

        let config = self.chain.query_to_scan_config(query).await?;
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
        match self.config.model.as_str() {
            m if m.contains("gpt-4o-mini") => 0.0001,
            m if m.contains("gpt-4o") => 0.001,
            m if m.contains("gpt-4") => 0.003,
            m if m.contains("gpt-3.5") => 0.00005,
            m if m.contains("claude-3-opus") => 0.015,
            m if m.contains("claude-3-sonnet") => 0.003,
            m if m.contains("claude-3-haiku") => 0.00025,
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
    pub async fn active_provider_name(&self) -> String {
        self.chain.active_provider_name().await
    }

    /// Get cache size.
    pub fn cache_size(&self) -> usize {
        self.cache.as_ref().map(|c| c.len()).unwrap_or(0)
    }

    /// Get number of providers in fallback chain.
    pub fn provider_count(&self) -> usize {
        self.chain.len()
    }

    /// Get provider kinds in the chain.
    pub fn provider_kinds(&self) -> Vec<ProviderKind> {
        self.chain.provider_kinds()
    }
}

/// Default LlmConfig implementation helper.
trait DefaultLlmConfig {
    fn default_config() -> LlmConfig;
}

impl DefaultLlmConfig for LlmConfig {
    fn default_config() -> LlmConfig {
        LlmConfig {
            enabled: false,
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: String::new(),
            max_calls_per_scan: 50,
            timeout_seconds: 30,
            cache_decisions: true,
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_llm_engine_creation() {
        let config = create_test_config("openai", true, true);
        let engine = LlmEngine::new(config);
        assert_eq!(engine.calls_made(), 0);
        assert!(engine.has_budget());
        assert_eq!(engine.provider_count(), 1);
    }

    #[tokio::test]
    async fn test_llm_engine_disabled() {
        let config = create_test_config("openai", false, true);
        let engine = LlmEngine::new(config);
        assert!(!engine.has_budget());
    }

    #[tokio::test]
    async fn test_llm_budget_tracking() {
        let config = create_test_config("openai", true, false);
        let engine = LlmEngine::new(config);
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

        assert_eq!(cache.len(), 2);
    }

    #[tokio::test]
    async fn test_cost_estimation_gpt4o_mini() {
        let config = create_test_config("openai", true, false);
        let engine = LlmEngine::new(config);
        assert_eq!(engine.estimated_cost(), 0.0);
    }

    #[tokio::test]
    async fn test_cost_estimation_claude() {
        let mut config = create_test_config("anthropic", true, false);
        config.model = "claude-3-haiku-20240307".to_string();
        let engine = LlmEngine::new(config);
        // Cost estimation is per-call, starts at 0
        assert_eq!(engine.estimated_cost(), 0.0);
    }

    #[tokio::test]
    async fn test_fallback_chain_creation() {
        let configs = vec![
            create_test_config("openai", true, true),
            create_test_config("ollama", true, true),
        ];
        let engine = LlmEngine::with_fallback(configs);
        assert_eq!(engine.provider_count(), 2);
    }

    #[tokio::test]
    async fn test_key_store_integration() {
        let temp_dir = TempDir::new().unwrap();
        let key_path = temp_dir.path().join("keys.json");

        let config = create_test_config("openai", true, true);
        let engine = LlmEngine::new(config).with_key_store(key_path.clone());

        engine.store_api_key("openai", "sk-test-123").unwrap();
        let key = engine.get_api_key("openai").unwrap().unwrap();
        assert_eq!(key, "sk-test-123");
    }

    #[tokio::test]
    async fn test_provider_kinds() {
        let config = create_test_config("openai", true, true);
        let engine = LlmEngine::new(config);
        let kinds = engine.provider_kinds();
        assert_eq!(kinds, vec![ProviderKind::OpenAi]);
    }

    #[tokio::test]
    async fn test_active_provider_name() {
        let config = create_test_config("openai", true, true);
        let engine = LlmEngine::new(config);
        let name = engine.active_provider_name().await;
        assert!(!name.is_empty());
    }
}
