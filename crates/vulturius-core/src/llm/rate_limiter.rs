//! Rate limiting for LLM API calls.
//!
//! Implements a token bucket algorithm with per-provider rate limits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;

/// Rate limit configuration for a provider.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per minute.
    pub requests_per_minute: u32,
    /// Maximum tokens per minute (0 = unlimited).
    pub tokens_per_minute: u64,
    /// Maximum concurrent requests.
    pub max_concurrent: u32,
}

impl RateLimitConfig {
    /// Default rate limits for OpenAI (gpt-4o-mini tier).
    pub fn openai_default() -> Self {
        Self {
            requests_per_minute: 500,
            tokens_per_minute: 150_000,
            max_concurrent: 10,
        }
    }

    /// Default rate limits for Anthropic.
    pub fn anthropic_default() -> Self {
        Self {
            requests_per_minute: 50,
            tokens_per_minute: 50_000,
            max_concurrent: 5,
        }
    }

    /// No rate limits (local providers like Ollama).
    pub fn unlimited() -> Self {
        Self {
            requests_per_minute: u32::MAX,
            tokens_per_minute: u64::MAX,
            max_concurrent: 100,
        }
    }
}

/// Token bucket for rate limiting.
struct TokenBucket {
    /// Maximum tokens in the bucket.
    capacity: u64,
    /// Current tokens available.
    tokens: u64,
    /// Refill rate (tokens per second).
    refill_rate: f64,
    /// Last refill time.
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u64, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume tokens. Returns true if successful.
    fn try_consume(&mut self, amount: u64) -> bool {
        self.refill();
        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    /// Calculate wait time for a given number of tokens.
    #[allow(dead_code)]
    fn wait_time_for(&self, amount: u64) -> Duration {
        let needed = amount.saturating_sub(self.tokens);
        if needed == 0 {
            return Duration::from_secs(0);
        }
        Duration::from_secs_f64(needed as f64 / self.refill_rate)
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = (elapsed * self.refill_rate) as u64;
        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;
    }
}

/// A rate limiter that tracks usage per provider.
pub struct RateLimiter {
    state: Arc<Mutex<RateLimiterState>>,
}

struct RateLimiterState {
    /// Per-provider rate limit configs.
    configs: HashMap<String, RateLimitConfig>,
    /// Per-provider request buckets (requests per minute).
    request_buckets: HashMap<String, TokenBucket>,
    /// Per-provider token buckets (tokens per minute).
    token_buckets: HashMap<String, TokenBucket>,
    /// Per-provider concurrent request counters.
    concurrent_counts: HashMap<String, u32>,
}

impl RateLimiter {
    /// Create a new rate limiter with default configs.
    pub fn new() -> Self {
        let mut state = RateLimiterState {
            configs: HashMap::new(),
            request_buckets: HashMap::new(),
            token_buckets: HashMap::new(),
            concurrent_counts: HashMap::new(),
        };

        // Set up defaults for known providers
        state
            .configs
            .insert("openai".to_string(), RateLimitConfig::openai_default());
        state
            .configs
            .insert("anthropic".to_string(), RateLimitConfig::anthropic_default());
        state
            .configs
            .insert("ollama".to_string(), RateLimitConfig::unlimited());

        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Configure rate limits for a provider (async).
    pub async fn set_config(&self, provider: &str, config: RateLimitConfig) {
        self.set_config_sync(provider, config);
    }

    /// Configure rate limits for a provider (synchronous).
    pub fn set_config_sync(&self, provider: &str, config: RateLimitConfig) {
        let mut state = self.state.lock().unwrap();
        let rpm = config.requests_per_minute as f64;
        let tpm = config.tokens_per_minute as f64;

        state.configs.insert(provider.to_string(), config.clone());
        state.request_buckets.insert(
            provider.to_string(),
            TokenBucket::new(config.requests_per_minute as u64, rpm / 60.0),
        );
        state.token_buckets.insert(
            provider.to_string(),
            TokenBucket::new(config.tokens_per_minute, tpm / 60.0),
        );
        state.concurrent_counts.insert(provider.to_string(), 0);
    }

    /// Wait until a request can be made to the given provider.
    pub async fn acquire(&self, provider: &str, estimated_tokens: u64) {
        loop {
            let can_proceed = self.try_acquire(provider, estimated_tokens).await;
            if can_proceed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn try_acquire(&self, provider: &str, estimated_tokens: u64) -> bool {
        let mut state = self.state.lock().unwrap();
        let config = state
            .configs
            .get(provider)
            .cloned()
            .unwrap_or(RateLimitConfig::unlimited());

        let provider_key = provider.to_string();

        // Check concurrent limit
        {
            let concurrent_count = state
                .concurrent_counts
                .entry(provider_key.clone())
                .or_insert(0);
            if *concurrent_count >= config.max_concurrent {
                debug!(
                    provider,
                    concurrent = *concurrent_count,
                    max = config.max_concurrent,
                    "Rate limiter: concurrent limit reached"
                );
                return false;
            }
        }

        // Check request bucket
        {
            let rpm = config.requests_per_minute as f64;
            let req_bucket = state
                .request_buckets
                .entry(provider_key.clone())
                .or_insert_with(|| {
                    TokenBucket::new(config.requests_per_minute as u64, rpm / 60.0)
                });

            if !req_bucket.try_consume(1) {
                debug!(provider, "Rate limiter: request limit reached");
                return false;
            }
        }

        if config.tokens_per_minute > 0 {
            // Check token bucket
            {
                let tpm = config.tokens_per_minute as f64;
                let tok_bucket = state
                    .token_buckets
                    .entry(provider_key.clone())
                    .or_insert_with(|| {
                        TokenBucket::new(config.tokens_per_minute, tpm / 60.0)
                    });

                if !tok_bucket.try_consume(estimated_tokens) {
                    debug!(provider, "Rate limiter: token limit reached");
                    return false;
                }
            }
        }

        // Increment concurrent count
        if let Some(count) = state.concurrent_counts.get_mut(&provider_key) {
            *count += 1;
        }
        true
    }

    /// Release a request slot for the given provider.
    pub async fn release(&self, provider: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(count) = state.concurrent_counts.get_mut(provider) {
            *count = count.saturating_sub(1);
        }
    }

    /// Clone for sharing across tasks.
    pub fn clone_shared(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_configs() {
        let openai = RateLimitConfig::openai_default();
        assert_eq!(openai.requests_per_minute, 500);

        let anthropic = RateLimitConfig::anthropic_default();
        assert_eq!(anthropic.requests_per_minute, 50);

        let unlimited = RateLimitConfig::unlimited();
        assert_eq!(unlimited.requests_per_minute, u32::MAX);
    }

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(100, 10.0); // 100 capacity, 10/sec refill
        assert!(bucket.try_consume(50));
        assert_eq!(bucket.tokens, 50);
        assert!(bucket.try_consume(50));
        assert!(!bucket.try_consume(1)); // Empty
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(100, 100.0); // 100/sec refill
        bucket.try_consume(100); // Drain
        assert_eq!(bucket.tokens, 0);

        // Simulate time passing
        bucket.last_refill = Instant::now() - Duration::from_millis(500);
        bucket.refill();
        assert!(bucket.tokens > 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_acquire_release() {
        let limiter = RateLimiter::new();
        limiter.acquire("openai", 100).await;
        limiter.release("openai").await;
    }

    #[tokio::test]
    async fn test_rate_limiter_clone() {
        let limiter = RateLimiter::new();
        let limiter2 = limiter.clone();
        limiter.acquire("openai", 100).await;
        limiter2.release("openai").await;
    }

    #[tokio::test]
    async fn test_rate_limiter_custom_config() {
        let limiter = RateLimiter::new();
        let config = RateLimitConfig {
            requests_per_minute: 10,
            tokens_per_minute: 1000,
            max_concurrent: 2,
        };
        limiter.set_config("custom", config).await;
        limiter.acquire("custom", 100).await;
        limiter.release("custom").await;
    }
}
