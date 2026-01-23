// src/provider/retry.rs
use anyhow::{bail, Result};
use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

use crate::color::{styled, warning_style};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not including the initial request)
    pub max_retries: u32,
    /// Base delay for exponential backoff (in milliseconds)
    pub base_delay_ms: u64,
    /// Maximum delay cap (in milliseconds)
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

/// Common API error response format
#[derive(Debug, Deserialize)]
struct ApiError {
    error: Option<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: Option<String>,
}

/// Try to extract a user-friendly error message from an API response body
fn extract_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<ApiError>(body)
        .ok()
        .and_then(|e| e.error)
        .and_then(|d| d.message)
}

/// Determines if a status code should trigger a retry
pub fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::INTERNAL_SERVER_ERROR
        || status == StatusCode::BAD_GATEWAY
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::GATEWAY_TIMEOUT
}

/// Calculates delay with exponential backoff and jitter
pub fn calculate_delay(attempt: u32, config: &RetryConfig) -> Duration {
    let exp_delay = config.base_delay_ms.saturating_mul(1 << attempt);
    let capped_delay = exp_delay.min(config.max_delay_ms);

    // Add jitter: random value between 0 and 50% of the delay
    let jitter = (rand_simple() * 0.5 * capped_delay as f64) as u64;

    Duration::from_millis(capped_delay + jitter)
}

/// Simple pseudo-random number generator (0.0 to 1.0)
/// Avoids adding rand crate as dependency
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    let mixed = nanos.wrapping_mul(1103515245).wrapping_add(12345);
    (mixed as f64) / (u32::MAX as f64)
}

/// Format a user-friendly error message based on status code
pub fn format_retry_error(status: StatusCode, attempt: u32, max_retries: u32) -> String {
    let status_desc = match status {
        StatusCode::TOO_MANY_REQUESTS => "Rate limited (429)",
        StatusCode::INTERNAL_SERVER_ERROR => "Server error (500)",
        StatusCode::BAD_GATEWAY => "Bad gateway (502)",
        StatusCode::SERVICE_UNAVAILABLE => "Service unavailable (503)",
        StatusCode::GATEWAY_TIMEOUT => "Gateway timeout (504)",
        _ => "Error",
    };

    if attempt < max_retries {
        format!(
            "{} - retrying ({}/{})",
            status_desc,
            attempt + 1,
            max_retries
        )
    } else {
        format!("{} - all {} retries exhausted", status_desc, max_retries)
    }
}

/// Check response and return appropriate error if retryable
pub fn check_response_for_retry(
    status: StatusCode,
    body: &str,
    attempt: u32,
    config: &RetryConfig,
) -> Result<RetryDecision> {
    if status.is_success() {
        return Ok(RetryDecision::Success);
    }

    if is_retryable_status(status) && attempt < config.max_retries {
        let delay = calculate_delay(attempt, config);
        eprintln!(
            "{} {}",
            styled("[WARN]", warning_style()),
            format_retry_error(status, attempt, config.max_retries)
        );
        return Ok(RetryDecision::Retry { delay });
    }

    // Non-retryable error or retries exhausted
    let error_msg = extract_error_message(body).unwrap_or_else(|| {
        let truncated = if body.len() > 500 { &body[..500] } else { body };
        truncated.to_string()
    });

    bail!("API error ({}): {}", status, error_msg)
}

#[derive(Debug)]
pub enum RetryDecision {
    Success,
    Retry { delay: Duration },
}

// =============================================================================
// MODULE TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Retryable status tests
    // ==========================================================================

    #[test]
    fn is_retryable_status_codes() {
        // Retryable
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));

        // Not retryable
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::FORBIDDEN));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
        assert!(!is_retryable_status(StatusCode::OK));
    }

    // ==========================================================================
    // Delay calculation tests
    // ==========================================================================

    #[test]
    fn calculate_delay_increases_exponentially() {
        let config = RetryConfig::default();
        let delay0 = calculate_delay(0, &config);
        let delay1 = calculate_delay(1, &config);
        let delay2 = calculate_delay(2, &config);

        // Base delays should roughly double (allowing for jitter)
        assert!(delay0.as_millis() >= 1000);
        assert!(delay1.as_millis() >= 2000);
        assert!(delay2.as_millis() >= 4000);
    }

    #[test]
    fn calculate_delay_respects_cap() {
        let config = RetryConfig {
            max_retries: 10,
            base_delay_ms: 1000,
            max_delay_ms: 5000,
        };
        let delay = calculate_delay(10, &config);
        // Should be capped at max_delay_ms + up to 50% jitter
        assert!(delay.as_millis() <= 7500);
    }

    // ==========================================================================
    // Config tests
    // ==========================================================================

    #[test]
    fn default_config_values() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
    }

    // ==========================================================================
    // Error formatting tests
    // ==========================================================================

    #[test]
    fn format_retry_error_during_retry() {
        let msg = format_retry_error(StatusCode::TOO_MANY_REQUESTS, 0, 3);
        assert!(msg.contains("Rate limited"));
        assert!(msg.contains("1/3"));
    }

    #[test]
    fn format_retry_error_exhausted() {
        let msg = format_retry_error(StatusCode::TOO_MANY_REQUESTS, 3, 3);
        assert!(msg.contains("exhausted"));
    }

    // ==========================================================================
    // Response check tests
    // ==========================================================================

    #[test]
    fn check_response_success() {
        let config = RetryConfig::default();
        let result = check_response_for_retry(StatusCode::OK, "", 0, &config);
        assert!(matches!(result.unwrap(), RetryDecision::Success));
    }

    #[test]
    fn check_response_retryable() {
        let config = RetryConfig::default();
        let result =
            check_response_for_retry(StatusCode::TOO_MANY_REQUESTS, "rate limited", 0, &config);
        assert!(matches!(result.unwrap(), RetryDecision::Retry { .. }));
    }

    #[test]
    fn check_response_non_retryable() {
        let config = RetryConfig::default();
        let result = check_response_for_retry(StatusCode::BAD_REQUEST, "bad request", 0, &config);
        assert!(result.is_err());
    }

    #[test]
    fn check_response_retries_exhausted() {
        let config = RetryConfig::default();
        let result =
            check_response_for_retry(StatusCode::TOO_MANY_REQUESTS, "rate limited", 3, &config);
        assert!(result.is_err());
    }

    #[test]
    fn check_response_uses_extracted_message() {
        let config = RetryConfig::default();
        let body = r#"{"error": {"message": "Quota exceeded"}}"#;
        let result = check_response_for_retry(StatusCode::BAD_REQUEST, body, 0, &config);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Quota exceeded"));
    }

    #[test]
    fn check_response_returns_error_on_exhaustion_with_message() {
        let config = RetryConfig {
            max_retries: 0,
            ..Default::default()
        };
        let body = r#"{"error": {"message": "Rate limit exceeded"}}"#;
        let result = check_response_for_retry(StatusCode::TOO_MANY_REQUESTS, body, 0, &config);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Rate limit exceeded"));
    }

    // ==========================================================================
    // Error message extraction tests
    // ==========================================================================

    #[test]
    fn extract_error_message_parses_json() {
        let body = r#"{"error": {"message": "Invalid API key"}}"#;
        assert_eq!(
            extract_error_message(body),
            Some("Invalid API key".to_string())
        );
    }

    #[test]
    fn extract_error_message_handles_missing_fields() {
        assert!(extract_error_message(r#"{"error": {}}"#).is_none());
        assert!(extract_error_message(r#"{"error": null}"#).is_none());
        assert!(extract_error_message("not json").is_none());
    }

    // ==========================================================================
    // Random tests
    // ==========================================================================

    #[test]
    fn rand_simple_in_range() {
        for _ in 0..100 {
            let r = rand_simple();
            assert!((0.0..=1.0).contains(&r));
        }
    }
}
