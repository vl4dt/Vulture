//! Retry logic with exponential backoff and jitter.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial attempt).
    pub max_retries: u32,
    /// Base delay between retries.
    pub base_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Backoff multiplier.
    pub backoff_multiplier: f64,
    /// Whether to add jitter to delays.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Retry an async operation with exponential backoff.
pub async fn retry<F, Fut, T, E>(config: &RetryConfig, mut operation: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempt = 0;

    loop {
        let result = operation().await;

        match result {
            Ok(value) => return Ok(value),
            Err(err) => {
                if attempt >= config.max_retries {
                    warn!(
                        attempt,
                        max_retries = config.max_retries,
                        "All retry attempts exhausted: {:?}",
                        err
                    );
                    return Err(err);
                }

                let delay = calculate_delay(config, attempt);
                debug!(
                    attempt,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis(),
                    "Retry attempt failed, backing off: {:?}",
                    err
                );

                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Calculate delay with exponential backoff and optional jitter.
fn calculate_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let exp = attempt.min(16) as f64; // Cap exponent to prevent overflow
    let delay = config.base_delay.as_millis() as f64 * config.backoff_multiplier.powf(exp);
    let delay = delay.min(config.max_delay.as_millis() as f64);

    let delay = if config.jitter {
        // Add ±25% random jitter
        let jitter_range = delay * 0.25;
        let jitter = (rand::random::<f64>() * 2.0 - 1.0) * jitter_range;
        (delay + jitter).max(0.0)
    } else {
        delay
    };

    Duration::from_millis(delay as u64)
}

/// Check if an HTTP status code indicates a retryable error.
pub fn is_retryable(status: reqwest::StatusCode) -> bool {
    status == 429 || // Too Many Requests
    status == 500 || // Internal Server Error
    status == 502 || // Bad Gateway
    status == 503 || // Service Unavailable
    status == 504   // Gateway Timeout
}

/// Extract Retry-After header value as Duration.
pub fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_secs(1));
        assert!(config.jitter);
    }

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(reqwest::StatusCode::from_u16(429).unwrap()));
        assert!(is_retryable(reqwest::StatusCode::from_u16(500).unwrap()));
        assert!(is_retryable(reqwest::StatusCode::from_u16(503).unwrap()));
        assert!(!is_retryable(reqwest::StatusCode::from_u16(200).unwrap()));
        assert!(!is_retryable(reqwest::StatusCode::from_u16(400).unwrap()));
        assert!(!is_retryable(reqwest::StatusCode::from_u16(401).unwrap()));
        assert!(!is_retryable(reqwest::StatusCode::from_u16(403).unwrap()));
    }

    #[tokio::test]
    async fn test_retry_succeeds_immediately() {
        let config = RetryConfig::default();
        let result = retry(&config, || async { Ok::<_, String>(42) }).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let config = RetryConfig {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            backoff_multiplier: 1.5,
            jitter: false,
        };

        let mut attempts = 0;
        let result = retry(&config, || {
            attempts += 1;
            async move {
                if attempts < 3 {
                    Err::<(), String>("not yet".to_string())
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
    }

    #[tokio::test]
    async fn test_retry_exhausts() {
        let config = RetryConfig {
            max_retries: 2,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(50),
            backoff_multiplier: 1.5,
            jitter: false,
        };

        let result = retry(&config, || async {
            Err::<(), String>("always fails".to_string())
        })
        .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_delay_calculation() {
        let config = RetryConfig {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let d0 = calculate_delay(&config, 0);
        let d1 = calculate_delay(&config, 1);
        let d2 = calculate_delay(&config, 2);

        assert_eq!(d0, Duration::from_secs(1));
        assert_eq!(d1, Duration::from_secs(2));
        assert_eq!(d2, Duration::from_secs(4));
    }

    #[test]
    fn test_delay_caps_at_max() {
        let config = RetryConfig {
            max_retries: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let delay = calculate_delay(&config, 10);
        assert!(delay <= Duration::from_secs(10));
    }
}
