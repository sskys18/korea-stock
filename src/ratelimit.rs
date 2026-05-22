use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

/// 토큰버킷 레이트리미터. 모든 REST 호출 전 `acquire().await`.
pub struct RateLimiter {
    inner: Mutex<Bucket>,
}

struct Bucket {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last: Instant,
}

impl RateLimiter {
    /// `per_sec` = 초당 허용 요청 수. 버킷 용량도 동일.
    pub fn new(per_sec: u32) -> Self {
        let cap = per_sec.max(1) as f64;
        Self {
            inner: Mutex::new(Bucket {
                capacity: cap,
                tokens: cap,
                refill_per_sec: cap,
                last: Instant::now(),
            }),
        }
    }

    /// 토큰 1개 확보까지 대기.
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut b = self.inner.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(b.last).as_secs_f64();
                b.tokens = (b.tokens + elapsed * b.refill_per_sec).min(b.capacity);
                b.last = now;
                if b.tokens >= 1.0 {
                    b.tokens -= 1.0;
                    return;
                }
                let deficit = 1.0 - b.tokens;
                Duration::from_secs_f64(deficit / b.refill_per_sec)
            };
            sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn burst_then_throttle() {
        // 2 req/s: 첫 2건 즉시, 3번째는 약 0.5s 대기.
        let rl = RateLimiter::new(2);
        let start = Instant::now();
        rl.acquire().await;
        rl.acquire().await;
        assert!(
            start.elapsed() < Duration::from_millis(100),
            "버스트 즉시 통과"
        );
        rl.acquire().await;
        assert!(
            start.elapsed() >= Duration::from_millis(400),
            "3번째 throttle"
        );
    }
}
