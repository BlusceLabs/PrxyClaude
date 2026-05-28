use std::collections::HashMap;

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
    
    /// Check if a request is allowed
    pub fn is_allowed(&mut self, client_id: &str) -> bool {
        let now = chrono::Utc::now();
        let minute_start = now - chrono::Duration::minutes(1);
        
        let requests = self.request_count.entry(client_id.to_string()).or_insert_with(Vec::new);
        
        // Remove older than 1 minute
        requests.retain(|&timestamp| timestamp > minute_start);
        
        if requests.len() >= self.requests_per_minute {
            return false;
        }
        
        requests.push(now);
        true
    }
    
    /// Reset rate limit for a client
    pub fn reset(&mut self, client_id: &str) {
        self.request_count.remove(client_id);
    }
    
    /// Get current request count for a client
    pub fn get_request_count(&self, client_id: &str) -> usize {
        self.request_count.get(client_id).map(|v| v.len()).unwrap_or(0)
    }
}

/// Global rate limiter instance
pub static mut GLOBAL_RATE_LIMITER: Option<RateLimiter> = None;

/// Initialize global rate limiter
pub fn init_rate_limiter(requests_per_minute: usize) {
    unsafe {
        GLOBAL_RATE_LIMITER = Some(RateLimiter::new(requests_per_minute));
    }
}

/// Check if a request is globally allowed
pub fn check_rate_limit(client_id: &str) -> bool {
    unsafe {
        if let Some(ref mut limiter) = GLOBAL_RATE_LIMITER {
            limiter.is_allowed(client_id)
        } else {
            true
        }
    }
}

/// Reset global rate limit
pub fn reset_rate_limit(client_id: &str) {
    unsafe {
        if let Some(ref mut limiter) = GLOBAL_RATE_LIMITER {
            limiter.reset(client_id);
        }
    }
}