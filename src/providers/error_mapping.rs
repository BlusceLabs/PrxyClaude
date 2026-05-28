use reqwest::StatusCode;

use super::exceptions::ProviderError;
use crate::providers::GlobalRateLimiter;

fn get_user_facing_error_message(e: &dyn std::error::Error) -> String {
    let msg = e.to_string();
    if msg.is_empty() {
        "An unknown error occurred.".to_string()
    } else {
        msg
    }
}

pub fn user_visible_message_for_mapped_provider_error(
    mapped: &ProviderError,
    provider_name: &str,
    _read_timeout_s: Option<f64>,
) -> String {
    if mapped.status_code == 405 {
        return format!(
            "Upstream provider {} rejected the request method or endpoint (HTTP 405).",
            provider_name
        );
    }
    get_user_facing_error_message(mapped)
}

pub fn map_http_status_error(
    status: StatusCode,
    message: &str,
    rate_limiter: Option<&GlobalRateLimiter>,
) -> ProviderError {
    match status.as_u16() {
        401 | 403 => ProviderError::authentication(message),
        429 => {
            if let Some(limiter) = rate_limiter {
                limiter.set_blocked(60.0);
            }
            ProviderError::rate_limit(message)
        }
        400 => ProviderError::invalid_request(message),
        502 | 503 | 504 => ProviderError::overloaded(message),
        s if s >= 500 => ProviderError::api_error(message, s),
        s => ProviderError::api_error(message, s),
    }
}

pub fn map_openai_error(status: u16, message: &str, rate_limiter: Option<&GlobalRateLimiter>) -> ProviderError {
    match status {
        401 => ProviderError::authentication(message),
        429 => {
            if let Some(limiter) = rate_limiter {
                limiter.set_blocked(60.0);
            }
            ProviderError::rate_limit(message)
        }
        400 => ProviderError::invalid_request(message),
        500 => {
            let lower = message.to_lowercase();
            if lower.contains("overloaded") || lower.contains("capacity") {
                ProviderError::overloaded(message)
            } else {
                ProviderError::api_error(message, 500)
            }
        }
        s => ProviderError::api_error(message, s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_status_401() {
        let err = map_http_status_error(StatusCode::UNAUTHORIZED, "bad key", None);
        assert_eq!(err.status_code, 401);
        assert_eq!(err.error_type, "authentication_error");
    }

    #[test]
    fn test_map_http_status_403() {
        let err = map_http_status_error(StatusCode::FORBIDDEN, "forbidden", None);
        assert_eq!(err.status_code, 401);
        assert_eq!(err.error_type, "authentication_error");
    }

    #[test]
    fn test_map_http_status_429() {
        let err = map_http_status_error(StatusCode::TOO_MANY_REQUESTS, "too fast", None);
        assert_eq!(err.status_code, 429);
        assert_eq!(err.error_type, "rate_limit_error");
    }

    #[test]
    fn test_map_http_status_503() {
        let err = map_http_status_error(StatusCode::SERVICE_UNAVAILABLE, "down", None);
        assert_eq!(err.status_code, 529);
        assert_eq!(err.error_type, "overloaded_error");
    }

    #[test]
    fn test_map_http_status_500() {
        let err = map_http_status_error(StatusCode::INTERNAL_SERVER_ERROR, "oops", None);
        assert_eq!(err.status_code, 500);
        assert_eq!(err.error_type, "api_error");
    }

    #[test]
    fn test_map_http_status_405() {
        let err = map_http_status_error(StatusCode::METHOD_NOT_ALLOWED, "nope", None);
        assert_eq!(err.status_code, 405);
        assert_eq!(err.error_type, "api_error");
    }

    #[test]
    fn test_map_openai_error_401() {
        let err = map_openai_error(401, "bad token", None);
        assert_eq!(err.status_code, 401);
        assert_eq!(err.error_type, "authentication_error");
    }

    #[test]
    fn test_map_openai_error_with_overloaded() {
        let err = map_openai_error(500, "upstream overloaded", None);
        assert_eq!(err.status_code, 529);
        assert_eq!(err.error_type, "overloaded_error");
    }

    #[test]
    fn test_map_openai_error_generic_500() {
        let err = map_openai_error(500, "server error", None);
        assert_eq!(err.status_code, 500);
        assert_eq!(err.error_type, "api_error");
    }

    #[test]
    fn test_provider_error_to_anthropic_format() {
        let err = ProviderError::authentication("invalid API key");
        let json = err.to_anthropic_format();
        assert_eq!(json["type"].as_str(), Some("error"));
        assert_eq!(json["error"]["type"].as_str(), Some("authentication_error"));
        assert_eq!(json["error"]["message"].as_str(), Some("invalid API key"));
    }

    #[test]
    fn test_user_visible_message_405() {
        let err = ProviderError::api_error("nope", 405);
        let msg = user_visible_message_for_mapped_provider_error(&err, "test_provider", None);
        assert!(msg.contains("test_provider"));
        assert!(msg.contains("405"));
    }
}
