use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<RateLimitState>>,
    max_per_minute: u32,
}

struct RateLimitState {
    counters: HashMap<String, (u32, Instant)>,
}

impl RateLimiter {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            state: Arc::new(Mutex::new(RateLimitState {
                counters: HashMap::new(),
            })),
            max_per_minute,
        }
    }

    pub fn check(&self, key: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();

        let (count, window_start) = state.counters.entry(key.to_string()).or_insert((0, now));

        // Reset window if 1 minute passed
        if now.duration_since(*window_start).as_secs() >= 60 {
            *count = 0;
            *window_start = now;
        }

        if *count >= self.max_per_minute {
            return false;
        }

        *count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_allows_within_limit() {
        let rl = RateLimiter::new(3);
        assert!(rl.check("test"));
        assert!(rl.check("test"));
        assert!(rl.check("test"));
        assert!(!rl.check("test")); // blocked
    }

    #[test]
    fn test_rate_limit_separate_keys() {
        let rl = RateLimiter::new(2);
        assert!(rl.check("key_a"));
        assert!(rl.check("key_b"));
        assert!(rl.check("key_a"));
        assert!(!rl.check("key_a")); // blocked for a
        assert!(rl.check("key_b")); // still ok for b
    }
}
