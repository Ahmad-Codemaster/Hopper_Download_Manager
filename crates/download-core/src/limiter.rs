use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct LimiterState {
    tokens: f64,
    last_update: Instant,
}

#[derive(Clone, Debug)]
pub struct SpeedLimiter {
    limit_bytes_per_sec: Arc<AtomicU64>,
    state: Arc<Mutex<LimiterState>>,
}

impl SpeedLimiter {
    pub fn new(limit_bytes_per_sec: u64) -> Self {
        Self {
            limit_bytes_per_sec: Arc::new(AtomicU64::new(limit_bytes_per_sec)),
            state: Arc::new(Mutex::new(LimiterState {
                tokens: limit_bytes_per_sec as f64,
                last_update: Instant::now(),
            })),
        }
    }

    pub fn set_limit(&self, limit: u64) {
        self.limit_bytes_per_sec.store(limit, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        state.tokens = limit as f64;
        state.last_update = Instant::now();
    }

    pub fn get_limit(&self) -> u64 {
        self.limit_bytes_per_sec.load(Ordering::Relaxed)
    }

    pub async fn consume(&self, bytes: u64) {
        let limit = self.limit_bytes_per_sec.load(Ordering::Relaxed);
        if limit == 0 || bytes == 0 {
            return;
        }

        let wait_duration = {
            let mut state = self.state.lock().unwrap();
            let now = Instant::now();
            let elapsed = now.duration_since(state.last_update).as_secs_f64();
            state.last_update = now;

            let rate = limit as f64;
            // Replenish tokens proportional to elapsed time (capped at 1s of tokens)
            state.tokens = (state.tokens + elapsed * rate).min(rate);

            if state.tokens < bytes as f64 {
                let deficit = (bytes as f64) - state.tokens;
                state.tokens = 0.0;
                let wait_secs = deficit / rate;
                // Cap single sleep at 1.5s to avoid freezing
                Some(Duration::from_secs_f64(wait_secs.min(1.5)))
            } else {
                state.tokens -= bytes as f64;
                None
            }
        };

        if let Some(dur) = wait_duration {
            tokio::time::sleep(dur).await;
        }
    }
}

impl Default for SpeedLimiter {
    fn default() -> Self {
        Self::new(0) // 0 means unlimited by default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_speed_by_default() {
        let limiter = SpeedLimiter::default();
        assert_eq!(limiter.get_limit(), 0);
    }

    #[test]
    fn set_and_get_speed_limit() {
        let limiter = SpeedLimiter::new(1024 * 1024);
        assert_eq!(limiter.get_limit(), 1024 * 1024);
        limiter.set_limit(5 * 1024 * 1024);
        assert_eq!(limiter.get_limit(), 5 * 1024 * 1024);
    }

    #[tokio::test]
    async fn zero_limit_consumes_instantly() {
        let limiter = SpeedLimiter::new(0);
        let start = Instant::now();
        limiter.consume(1_000_000).await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn throttles_when_deficit_occurs() {
        // Limit: 50 KB/s
        let limiter = SpeedLimiter::new(50_000);
        // Deplete initial tokens
        limiter.consume(50_000).await;
        let start = Instant::now();
        // Request another 10 KB -> should take approx 200ms
        limiter.consume(10_000).await;
        assert!(start.elapsed() >= Duration::from_millis(150));
    }
}
