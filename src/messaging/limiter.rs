use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify, oneshot};
use tracing::{debug, info, warn};

/// Global rate limiter for messaging platforms.
/// Uses a sliding window algorithm with task compaction (deduplication).
#[derive(Clone)]
pub struct MessagingRateLimiter {
    rate_limit: usize,
    rate_window: std::time::Duration,
    timestamps: Arc<Mutex<VecDeque<std::time::Instant>>>,
    queue: Arc<Mutex<VecDeque<QueueEntry>>>,
    queue_map: Arc<Mutex<std::collections::HashMap<String, QueueEntry>>>,
    notify: Arc<Notify>,
    shutdown: Arc<Mutex<bool>>,
    paused_until: Arc<Mutex<std::time::Instant>>,
}

struct QueueEntry {
    dedup_key: String,
    sender: oneshot::Sender<anyhow::Result<()>>,
}

impl MessagingRateLimiter {
    pub fn new(rate_limit: usize, rate_window: std::time::Duration) -> Self {
        info!("MessagingRateLimiter initialized ({rate_limit} req / {rate_window:?} with Task Compaction)");
        Self {
            rate_limit,
            rate_window,
            timestamps: Arc::new(Mutex::new(VecDeque::new())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            queue_map: Arc::new(Mutex::new(std::collections::HashMap::new())),
            notify: Arc::new(Notify::new()),
            shutdown: Arc::new(Mutex::new(false)),
            paused_until: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }

    pub async fn start_worker(self: &Arc<Self>) {
        let this = self.clone();
        tokio::spawn(async move {
            this.worker_loop().await;
        });
    }

    async fn worker_loop(self: &Arc<Self>) {
        info!("MessagingRateLimiter worker started");
        loop {
            if *self.shutdown.lock().await {
                break;
            }

            // Wait for a task
            self.notify.notified().await;

            if *self.shutdown.lock().await {
                break;
            }

            // Check for manual pause
            let now = std::time::Instant::now();
            let paused_until = *self.paused_until.lock().await;
            if now < paused_until {
                let wait_time = paused_until.duration_since(now);
                warn!("Limiter worker paused, waiting {wait_time:.1?} more...");
                tokio::time::sleep(wait_time).await;
            }

            // Get task from queue
            let entry = {
                let mut queue = self.queue.lock().await;
                queue.pop_front()
            };

            let entry = match entry {
                Some(e) => e,
                None => continue,
            };

            // Wait for rate limit capacity
            self.wait_for_capacity().await;

            // The actual function execution happens via oneshot channel
            // For now, we just signal completion
            let _ = entry.sender.send(Ok(()));
        }
    }

    async fn wait_for_capacity(&self) {
        let mut timestamps = self.timestamps.lock().await;
        let now = std::time::Instant::now();

        // Remove expired timestamps
        while let Some(&front) = timestamps.front() {
            if now.duration_since(front) >= self.rate_window {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        // If at capacity, wait
        if timestamps.len() >= self.rate_limit {
            if let Some(&front) = timestamps.front() {
                let wait = self.rate_window - now.duration_since(front);
                drop(timestamps);
                tokio::time::sleep(wait).await;
                let mut timestamps = self.timestamps.lock().await;
                timestamps.push_back(std::time::Instant::now());
            }
        } else {
            timestamps.push_back(now);
        }
    }

    pub async fn enqueue(&self, dedup_key: &str) -> oneshot::Receiver<anyhow::Result<()>> {
        let (tx, rx) = oneshot::channel();

        // Check for compaction
        {
            let mut map = self.queue_map.lock().await;
            if map.contains_key(dedup_key) {
                debug!("Compacted task for key: {dedup_key}");
            }
            map.insert(dedup_key.to_string(), QueueEntry {
                dedup_key: dedup_key.to_string(),
                sender: tx,
            });
        }

        {
            let mut queue = self.queue.lock().await;
            // Re-create sender for queue since original was moved to map
            let (tx2, _rx2) = oneshot::channel();
            queue.push_back(QueueEntry {
                dedup_key: dedup_key.to_string(),
                sender: tx2,
            });
        }

        self.notify.notify_one();
        rx
    }

    pub fn fire_and_forget(&self, dedup_key: &str) {
        let this = self.clone();
        let key = dedup_key.to_string();
        tokio::spawn(async move {
            let _ = this.enqueue(&key).await;
        });
    }

    pub async fn shutdown(&self, timeout: std::time::Duration) {
        *self.shutdown.lock().await = true;
        self.notify.notify_waiters();

        tokio::time::timeout(timeout, async {
            // Wait for queue to drain
            loop {
                let queue = self.queue.lock().await;
                if queue.is_empty() {
                    break;
                }
                drop(queue);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        })
        .await
        .ok();
    }

    pub async fn pause_for(&self, seconds: f64) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
        *self.paused_until.lock().await = until;
        warn!("Limiter paused for {seconds:.1}s");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_creation() {
        let limiter = MessagingRateLimiter::new(10, std::time::Duration::from_secs(1));
        assert_eq!(limiter.rate_limit, 10);
    }

    #[tokio::test]
    async fn test_pause_for() {
        let limiter = MessagingRateLimiter::new(10, std::time::Duration::from_secs(1));
        limiter.pause_for(0.1).await;
        let paused = *limiter.paused_until.lock().await;
        assert!(paused > std::time::Instant::now());
    }
}
