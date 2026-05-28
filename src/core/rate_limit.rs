use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Simple rate limiter
pub struct RateLimiter {
    requests_per_minute: usize,
    request_count: HashMap<String, Vec<chrono::DateTime<chrono::Utc>>>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: usize) -> Self {
        Self {
            requests_per_minute,
            request_count: HashMap::new(),
        }
    }

    pub fn is_allowed(&mut self, client_id: &str) -> bool {
        let now = chrono::Utc::now();
        let minute_start = now - chrono::Duration::minutes(1);

        let requests = self
            .request_count
            .entry(client_id.to_string())
            .or_insert_with(Vec::new);

        requests.retain(|&timestamp| timestamp > minute_start);

        if requests.len() >= self.requests_per_minute {
            return false;
        }

        requests.push(now);
        true
    }

    pub fn reset(&mut self, client_id: &str) {
        self.request_count.remove(client_id);
    }

    pub fn get_request_count(&self, client_id: &str) -> usize {
        self.request_count
            .get(client_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

/// Global rate limiter instance
static GLOBAL_RATE_LIMITER: OnceLock<Mutex<RateLimiter>> = OnceLock::new();

/// Initialize global rate limiter
pub fn init_rate_limiter(requests_per_minute: usize) {
    let _ = GLOBAL_RATE_LIMITER.set(Mutex::new(RateLimiter::new(requests_per_minute)));
}

/// Check if a request is globally allowed
pub fn check_rate_limit(client_id: &str) -> bool {
    if let Some(limiter) = GLOBAL_RATE_LIMITER.get() {
        limiter.lock().unwrap().is_allowed(client_id)
    } else {
        true
    }
}

/// Reset global rate limit
pub fn reset_rate_limit(client_id: &str) {
    if let Some(limiter) = GLOBAL_RATE_LIMITER.get() {
        limiter.lock().unwrap().reset(client_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_first_request() {
        let mut limiter = RateLimiter::new(10);
        assert!(limiter.is_allowed("client1"));
    }

    #[test]
    fn test_rate_limiter_blocks_excess() {
        let mut limiter = RateLimiter::new(2);
        assert!(limiter.is_allowed("client1"));
        assert!(limiter.is_allowed("client1"));
        assert!(!limiter.is_allowed("client1"));
    }

    #[test]
    fn test_rate_limiter_allows_different_clients() {
        let mut limiter = RateLimiter::new(1);
        assert!(limiter.is_allowed("client1"));
        assert!(limiter.is_allowed("client2"));
    }

    #[test]
    fn test_rate_limiter_reset() {
        let mut limiter = RateLimiter::new(1);
        assert!(limiter.is_allowed("client1"));
        limiter.reset("client1");
        assert!(limiter.is_allowed("client1"));
    }

    #[test]
    fn test_get_request_count() {
        let mut limiter = RateLimiter::new(5);
        assert_eq!(limiter.get_request_count("client1"), 0);
        limiter.is_allowed("client1");
        limiter.is_allowed("client1");
        assert_eq!(limiter.get_request_count("client1"), 2);
    }
}