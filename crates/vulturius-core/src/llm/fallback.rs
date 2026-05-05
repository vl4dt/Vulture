//! Provider fallback chain.
//!
//! Tries multiple providers in order until one succeeds.

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::provider::openai::OpenAiProvider;
use super::provider::ollama::OllamaProvider;
use super::provider::ProviderKind;
use super::rate_limiter::RateLimiter;
use super::provider::LlmAnalysisResult;
use crate::config::LlmConfig;
use crate::types::FileEntry;
use crate::Result;

/// A single provider in the fallback chain.
pub enum ChainProvider {
    OpenAi(OpenAiProvider),
    Ollama(OllamaProvider),
}

/// Fallback chain that tries providers in order.
pub struct FallbackChain {
    /// Ordered list of providers to try.
    providers: Vec<ChainProvider>,
    /// Whether to continue trying providers on error.
    continue_on_error: bool,
    /// Current active provider index (for logging).
    active_index: Arc<Mutex<usize>>,
}

impl FallbackChain {
    /// Create a new fallback chain from configs.
    pub fn from_configs(
        configs: &[LlmConfig],
        rate_limiter: RateLimiter,
    ) -> Self {
        let mut providers = Vec::new();

        for config in configs {
            let provider = match config.provider.as_str() {
                "openai" => ChainProvider::OpenAi(OpenAiProvider::new(config, rate_limiter.clone())),
                "ollama" => ChainProvider::Ollama(OllamaProvider::new(config, rate_limiter.clone())),
                _ => {
                    warn!(provider = %config.provider, "Unknown provider, skipping");
                    continue;
                }
            };
            providers.push(provider);
        }

        Self {
            providers,
            continue_on_error: true,
            active_index: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a chain with a single provider.
    pub fn single(config: &LlmConfig, rate_limiter: RateLimiter) -> Self {
        Self::from_configs(&[config.clone()], rate_limiter)
    }

    /// Set whether to continue trying providers on error.
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Get the number of providers in the chain.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Analyze a single file, trying providers in order.
    pub async fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        if self.providers.is_empty() {
            return Ok(None);
        }

        let mut last_error = None;

        for (i, provider) in self.providers.iter().enumerate() {
            let provider_name = provider.name();
            debug!(
                provider = provider_name,
                index = i,
                total = self.providers.len(),
                "Trying provider"
            );

            match provider.analyze_file(entry).await {
                Ok(result) => {
                    // Update active provider index
                    let mut active = self.active_index.lock().await;
                    *active = i;

                    info!(
                        provider = provider_name,
                        index = i,
                        "Provider succeeded"
                    );
                    return Ok(result);
                }
                Err(e) => {
                    warn!(
                        provider = provider_name,
                        error = %e,
                        "Provider failed"
                    );
                    last_error = Some(e);

                    if !self.continue_on_error {
                        return Err(last_error.unwrap());
                    }
                }
            }
        }

        // All providers failed
        if let Some(e) = last_error {
            warn!("All providers in fallback chain failed");
            Err(e)
        } else {
            Ok(None)
        }
    }

    /// Analyze a batch of files, trying providers in order.
    pub async fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        if self.providers.is_empty() {
            return Ok(Vec::new());
        }

        let mut last_error = None;

        for (i, provider) in self.providers.iter().enumerate() {
            let provider_name = provider.name();
            debug!(
                provider = provider_name,
                index = i,
                total = self.providers.len(),
                "Trying provider for batch"
            );

            match provider.analyze_batch(entries).await {
                Ok(results) => {
                    let mut active = self.active_index.lock().await;
                    *active = i;

                    info!(
                        provider = provider_name,
                        index = i,
                        results = results.len(),
                        "Provider succeeded for batch"
                    );
                    return Ok(results);
                }
                Err(e) => {
                    warn!(
                        provider = provider_name,
                        error = %e,
                        "Provider failed for batch"
                    );
                    last_error = Some(e);

                    if !self.continue_on_error {
                        return Err(last_error.unwrap());
                    }
                }
            }
        }

        if let Some(e) = last_error {
            warn!("All providers in fallback chain failed for batch");
            Err(e)
        } else {
            Ok(Vec::new())
        }
    }

    /// Convert natural language query to scan config.
    pub async fn query_to_scan_config(&self, query: &str) -> Result<String> {
        if self.providers.is_empty() {
            return Err(crate::VulturiusError::Llm(
                "No providers configured".to_string(),
            ));
        }

        let mut last_error = None;

        for (i, provider) in self.providers.iter().enumerate() {
            let provider_name = provider.name();
            debug!(
                provider = provider_name,
                index = i,
                "Trying provider for NL query"
            );

            match provider.query_to_scan_config(query).await {
                Ok(config) => {
                    let mut active = self.active_index.lock().await;
                    *active = i;

                    info!(
                        provider = provider_name,
                        index = i,
                        "Provider succeeded for NL query"
                    );
                    return Ok(config);
                }
                Err(e) => {
                    warn!(
                        provider = provider_name,
                        error = %e,
                        "Provider failed for NL query"
                    );
                    last_error = Some(e);

                    if !self.continue_on_error {
                        return Err(last_error.unwrap());
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            crate::VulturiusError::Llm("All providers failed for NL query".to_string())
        }))
    }

    /// Get the name of the currently active provider.
    pub async fn active_provider_name(&self) -> String {
        let index = *self.active_index.lock().await;
        if index < self.providers.len() {
            self.providers[index].name().to_string()
        } else {
            "none".to_string()
        }
    }

    /// Get provider kinds in the chain.
    pub fn provider_kinds(&self) -> Vec<ProviderKind> {
        self.providers.iter().map(|p| p.kind()).collect()
    }
}

// Helper trait for provider name/kind
trait ProviderInfo {
    fn name(&self) -> &str;
    fn kind(&self) -> ProviderKind;
    fn analyze_file(&self, entry: &FileEntry) -> impl std::future::Future<Output = Result<Option<LlmAnalysisResult>>>;
    fn analyze_batch(&self, entries: &[FileEntry]) -> impl std::future::Future<Output = Result<Vec<LlmAnalysisResult>>>;
    fn query_to_scan_config(&self, query: &str) -> impl std::future::Future<Output = Result<String>>;
}

impl ProviderInfo for ChainProvider {
    fn name(&self) -> &str {
        match self {
            ChainProvider::OpenAi(p) => p.name(),
            ChainProvider::Ollama(p) => p.name(),
        }
    }

    fn kind(&self) -> ProviderKind {
        match self {
            ChainProvider::OpenAi(p) => p.kind(),
            ChainProvider::Ollama(p) => p.kind(),
        }
    }

    async fn analyze_file(&self, entry: &FileEntry) -> Result<Option<LlmAnalysisResult>> {
        match self {
            ChainProvider::OpenAi(p) => p.analyze_file(entry).await,
            ChainProvider::Ollama(p) => p.analyze_file(entry).await,
        }
    }

    async fn analyze_batch(&self, entries: &[FileEntry]) -> Result<Vec<LlmAnalysisResult>> {
        match self {
            ChainProvider::OpenAi(p) => p.analyze_batch(entries).await,
            ChainProvider::Ollama(p) => p.analyze_batch(entries).await,
        }
    }

    async fn query_to_scan_config(&self, query: &str) -> Result<String> {
        match self {
            ChainProvider::OpenAi(p) => p.query_to_scan_config(query).await,
            ChainProvider::Ollama(p) => p.query_to_scan_config(query).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::rate_limiter::RateLimiter;

    fn test_config(provider: &str) -> LlmConfig {
        LlmConfig {
            enabled: true,
            provider: provider.to_string(),
            model: "test-model".to_string(),
            api_key: "test-key".to_string(),
            max_calls_per_scan: 50,
            timeout_seconds: 30,
            cache_decisions: true,
        }
    }

    #[test]
    fn test_fallback_chain_creation() {
        let limiter = RateLimiter::new();
        let configs = vec![test_config("openai"), test_config("ollama")];
        let chain = FallbackChain::from_configs(&configs, limiter);
        assert_eq!(chain.len(), 2);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_fallback_chain_single() {
        let limiter = RateLimiter::new();
        let config = test_config("openai");
        let chain = FallbackChain::single(&config, limiter);
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_fallback_chain_empty() {
        let limiter = RateLimiter::new();
        let chain = FallbackChain::from_configs(&[], limiter);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_fallback_chain_provider_kinds() {
        let limiter = RateLimiter::new();
        let configs = vec![test_config("openai"), test_config("ollama")];
        let chain = FallbackChain::from_configs(&configs, limiter);
        let kinds = chain.provider_kinds();
        assert_eq!(kinds, vec![ProviderKind::OpenAi, ProviderKind::Ollama]);
    }
}
