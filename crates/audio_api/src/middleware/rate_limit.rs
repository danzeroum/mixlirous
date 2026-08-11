use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

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

    pub async fn check(&self, key: &str) -> bool {
        let mut state = self.state.lock().await;
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

/// Builds an axum middleware function that uses the given RateLimiter.
pub fn rate_limit_middleware(
    limiter: Arc<RateLimiter>,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>>
       + Clone
       + Send
       + 'static {
    move |req: Request, next: Next| {
        let limiter = limiter.clone();
        Box::pin(async move {
            let key = req
                .headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("unknown")
                .split(',')
                .next()
                .unwrap_or("unknown")
                .trim();

            if !limiter.check(key).await {
                return Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .body("rate limit exceeded".into())
                    .unwrap();
            }

            next.run(req).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_allows_within_limit() {
        let rl = RateLimiter::new(3);
        assert!(rl.check("test").await);
        assert!(rl.check("test").await);
        assert!(rl.check("test").await);
        assert!(!rl.check("test").await);
    }

    #[tokio::test]
    async fn test_rate_limit_separate_keys() {
        let rl = RateLimiter::new(2);
        assert!(rl.check("key_a").await);
        assert!(rl.check("key_b").await);
        assert!(rl.check("key_a").await);
        assert!(!rl.check("key_a").await);
        assert!(rl.check("key_b").await);
    }
}
