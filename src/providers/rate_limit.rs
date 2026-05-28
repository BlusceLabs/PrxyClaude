use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use tokio::sync::Semaphore;

pub struct StrictSlidingWindowLimiter {
    max_requests: usize,
    window_secs: u64,
    timestamps: Mutex<VecDeque<Instant>>,
}

impl StrictSlidingWindowLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            timestamps: Mutex::new(VecDeque::new()),
        }
    }

    pub fn acquire(&self) {
        loop {
            let now = Instant::now();
            let mut timestamps = self.timestamps.lock().unwrap();

            while let Some(&ts) = timestamps.front() {
                if now.duration_since(ts).as_secs() >= self.window_secs {
                    timestamps.pop_front();
                } else {
                    break;
                }
            }

            if timestamps.len() < self.max_requests {
                timestamps.push_back(now);
                return;
            }

            let sleep_until = timestamps.front().copied().unwrap()
                + std::time::Duration::from_secs(self.window_secs);
            let sleep_dur = sleep_until.saturating_duration_since(now);
            drop(timestamps);
            std::thread::sleep(sleep_dur);
        }
    }
}

static SCOPED_INSTANCES: LazyLock<Mutex<HashMap<String, Arc<GlobalRateLimiter>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct GlobalRateLimiter {
    rate_limit: usize,
    rate_window: f64,
    max_concurrency: usize,
    #[allow(dead_code)]
    proactive_limiter: StrictSlidingWindowLimiter,
    blocked_until: Mutex<Instant>,
    concurrency_sem: Arc<Semaphore>,
}

impl GlobalRateLimiter {
    pub fn new(rate_limit: usize, rate_window: f64, max_concurrency: usize) -> Self {
        assert!(rate_limit > 0, "rate_limit must be > 0");
        assert!(rate_window > 0.0, "rate_window must be > 0");
        assert!(max_concurrency > 0, "max_concurrency must be > 0");

        Self {
            rate_limit,
            rate_window,
            max_concurrency,
            proactive_limiter: StrictSlidingWindowLimiter::new(rate_limit, rate_window as u64),
            blocked_until: Mutex::new(Instant::now()),
            concurrency_sem: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    pub fn get_scoped_instance(
        scope: &str,
        rate_limit: Option<usize>,
        rate_window: Option<f64>,
        max_concurrency: usize,
    ) -> Arc<GlobalRateLimiter> {
        if scope.is_empty() {
            panic!("scope must be non-empty");
        }
        let desired_rate_limit = rate_limit.unwrap_or(40);
        let desired_rate_window = rate_window.unwrap_or(60.0);

        let mut instances = SCOPED_INSTANCES.lock().unwrap();
        if let Some(existing) = instances.get(scope) {
            if existing.matches_config(desired_rate_limit, desired_rate_window, max_concurrency) {
                return existing.clone();
            }
        }

        let limiter = Arc::new(GlobalRateLimiter::new(
            desired_rate_limit,
            desired_rate_window,
            max_concurrency,
        ));
        instances.insert(scope.to_string(), limiter.clone());
        limiter
    }

    pub fn matches_config(&self, rate_limit: usize, rate_window: f64, max_concurrency: usize) -> bool {
        self.rate_limit == rate_limit
            && (self.rate_window - rate_window).abs() < f64::EPSILON
            && self.max_concurrency == max_concurrency
    }

    pub fn set_blocked(&self, seconds: f64) {
        let mut blocked = self.blocked_until.lock().unwrap();
        *blocked = Instant::now() + std::time::Duration::from_secs_f64(seconds);
    }

    pub fn is_blocked(&self) -> bool {
        let blocked = self.blocked_until.lock().unwrap();
        Instant::now() < *blocked
    }

    pub fn remaining_wait(&self) -> f64 {
        let blocked = self.blocked_until.lock().unwrap();
        let remaining = blocked.saturating_duration_since(Instant::now());
        remaining.as_secs_f64()
    }

    pub async fn wait_if_blocked(&self) -> bool {
        let waited_reactively;
        let now = Instant::now();
        {
            let blocked = self.blocked_until.lock().unwrap();
            waited_reactively = now < *blocked;
        }
        if waited_reactively {
            let blocked = self.blocked_until.lock().unwrap();
            let wait_dur = blocked.saturating_duration_since(now);
            drop(blocked);
            if !wait_dur.is_zero() {
                tracing::warn!("Global provider rate limit active (reactive), waiting...");
                tokio::time::sleep(wait_dur).await;
            }
        }
        waited_reactively
    }

    pub async fn concurrency_slot(&self) -> tokio::sync::SemaphorePermit<'_> {
        self.concurrency_sem.acquire().await.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_creation() {
        let limiter = GlobalRateLimiter::new(10, 60.0, 5);
        assert!(!limiter.is_blocked());
        assert_eq!(limiter.remaining_wait(), 0.0);
    }

    #[test]
    fn test_set_blocked() {
        let limiter = GlobalRateLimiter::new(10, 60.0, 5);
        limiter.set_blocked(30.0);
        assert!(limiter.is_blocked());
        assert!(limiter.remaining_wait() > 0.0);
    }

    #[test]
    fn test_matches_config() {
        let limiter = GlobalRateLimiter::new(10, 60.0, 5);
        assert!(limiter.matches_config(10, 60.0, 5));
        assert!(!limiter.matches_config(20, 60.0, 5));
        assert!(!limiter.matches_config(10, 30.0, 5));
        assert!(!limiter.matches_config(10, 60.0, 10));
    }

    #[test]
    fn test_get_scoped_instance_caching() {
        let a = GlobalRateLimiter::get_scoped_instance("test_scope", Some(10), Some(60.0), 5);
        let b = GlobalRateLimiter::get_scoped_instance("test_scope", Some(10), Some(60.0), 5);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn test_get_scoped_instance_different_scopes() {
        let a = GlobalRateLimiter::get_scoped_instance("scope_a", Some(10), Some(60.0), 5);
        let b = GlobalRateLimiter::get_scoped_instance("scope_b", Some(10), Some(60.0), 5);
        assert!(!Arc::ptr_eq(&a, &b));
    }
}
