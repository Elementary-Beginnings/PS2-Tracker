//! Rate limit and backoff – considerate towards the Census API.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

/// Enforces a minimum interval between two requests (across all callers).
#[derive(Debug)]
pub struct RateLimiter {
    interval: Duration,
    next: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    /// Reserves the next free slot and waits until then.
    pub async fn acquire(&self) {
        let at = {
            let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let at = next.map_or(now, |n| n.max(now));
            *next = Some(at + self.interval);
            at
        };
        tokio::time::sleep_until(at).await;
    }
}

/// Exponential backoff with an upper bound and a deterministic jitter share.
#[derive(Debug, Clone)]
pub struct Backoff {
    base: Duration,
    max: Duration,
    attempt: u32,
    seed: u64,
}

impl Backoff {
    pub fn new(base: Duration, max: Duration) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(7);
        Self {
            base,
            max,
            attempt: 0,
            seed,
        }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Next delay: base·2^n, capped at `max`, then 75–100 % of it (jitter).
    pub fn next_delay(&mut self) -> Duration {
        let exp = self.base.saturating_mul(1u32 << self.attempt.min(16));
        let capped = exp.min(self.max);
        self.attempt = self.attempt.saturating_add(1);
        // xorshift for jitter, no extra dependency
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        let frac = 0.75 + (self.seed % 1000) as f64 / 4000.0;
        capped.mul_f64(frac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn limiter_spaces_requests() {
        let rl = RateLimiter::new(Duration::from_millis(250));
        let start = Instant::now();
        for _ in 0..5 {
            rl.acquire().await;
        }
        // first immediately, then 4 × 250 ms
        assert_eq!(start.elapsed(), Duration::from_millis(1000));
    }

    #[tokio::test(start_paused = true)]
    async fn limiter_does_not_accumulate_credit() {
        let rl = RateLimiter::new(Duration::from_millis(100));
        rl.acquire().await;
        tokio::time::sleep(Duration::from_secs(5)).await;
        let t = Instant::now();
        rl.acquire().await;
        rl.acquire().await;
        assert_eq!(t.elapsed(), Duration::from_millis(100));
    }

    #[test]
    fn backoff_grows_and_caps() {
        let mut b = Backoff::new(Duration::from_secs(1), Duration::from_secs(60)).with_seed(42);
        let d: Vec<_> = (0..10).map(|_| b.next_delay()).collect();
        assert!(d[0] >= Duration::from_millis(750) && d[0] <= Duration::from_secs(1));
        assert!(d[3] >= Duration::from_secs(6) && d[3] <= Duration::from_secs(8));
        assert!(d.iter().all(|x| *x <= Duration::from_secs(60)));
        assert!(d[9] >= Duration::from_secs(45));
        b.reset();
        assert!(b.next_delay() <= Duration::from_secs(1));
    }
}
