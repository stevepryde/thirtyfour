use crate::support::sleep;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

/// Trait for implementing the element polling strategy.
///
/// Each time the element condition is not met, the `tick()` method will be
/// called. Upon returning `false`, the polling loop will terminate.
pub trait ElementPoller: Debug + Send + 'static {
    /// Process the poller forward by one tick.
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}

/// Trait for returning a struct that implements `ElementPoller`.
///
/// The `start()` method will be called at the beginning of the polling loop.
pub trait IntoElementPoller: Debug {
    /// Start a new poller.
    fn start(&self) -> AnyElementPoller;
}

/// Enum wrapper to enable static dispatch instead of dynamic Box<dyn>.
#[derive(Debug, Clone)]
pub enum AnyElementPoller {
    /// The with-timeout variant.
    WithTimeout(ElementPollerWithTimeout),
    /// The no-wait (single attempt) variant.
    NoWait(ElementPollerNoWait),
}

impl ElementPoller for AnyElementPoller {
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        match self {
            Self::WithTimeout(p) => p.tick(),
            Self::NoWait(p) => p.tick(),
        }
    }
}

impl Default for AnyElementPoller {
    fn default() -> Self {
        Self::WithTimeout(ElementPollerWithTimeout::default())
    }
}

impl From<ElementPollerWithTimeout> for AnyElementPoller {
    fn from(p: ElementPollerWithTimeout) -> Self {
        Self::WithTimeout(p)
    }
}

impl From<ElementPollerNoWait> for AnyElementPoller {
    fn from(p: ElementPollerNoWait) -> Self {
        Self::NoWait(p)
    }
}

/// Poll up to the specified timeout, with the specified interval being the
/// minimum time elapsed between the start of each poll attempt.
/// If the previous poll attempt took longer than the interval, the next will
/// start immediately. Once the timeout is reached, a Timeout error will be
/// returned regardless of the actual number of polling attempts completed.
#[derive(Debug, Clone)]
pub struct ElementPollerWithTimeout {
    timeout: Duration,
    interval: Duration,
    start: Instant,
    cur_tries: u32,
}

impl ElementPollerWithTimeout {
    /// Create a new `ElementPollerWithTimeout`.
    #[must_use] 
    pub fn new(timeout: Duration, interval: Duration) -> Self {
        Self {
            timeout,
            interval,
            start: Instant::now(),
            cur_tries: 0,
        }
    }
}

impl Default for ElementPollerWithTimeout {
    fn default() -> Self {
        Self::new(Duration::from_secs(20), Duration::from_millis(500))
    }
}

impl ElementPoller for ElementPollerWithTimeout {
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let timeout = self.timeout;
        let interval = self.interval;

        // Capture mutable state before async block
        let cur_tries = self.cur_tries;
        let start = self.start;

        // Increment for next call
        self.cur_tries += 1;

        Box::pin(async move {
            if start.elapsed() >= timeout {
                return false;
            }

            // The Next poll is due no earlier than this long after the first poll started.
            let minimum_elapsed = interval.saturating_mul(cur_tries + 1);

            // But this much time has elapsed since the first poll started.
            let actual_elapsed = start.elapsed();

            if actual_elapsed < minimum_elapsed {
                sleep(minimum_elapsed.checked_sub(actual_elapsed).unwrap()).await;
            }

            true
        })
    }
}

impl IntoElementPoller for ElementPollerWithTimeout {
    fn start(&self) -> AnyElementPoller {
        AnyElementPoller::WithTimeout(ElementPollerWithTimeout::new(self.timeout, self.interval))
    }
}

/// No polling, single attempt.
#[derive(Debug, Clone)]
pub struct ElementPollerNoWait;

impl ElementPoller for ElementPollerNoWait {
    fn tick(&mut self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async move { false })
    }
}

impl IntoElementPoller for ElementPollerNoWait {
    fn start(&self) -> AnyElementPoller {
        AnyElementPoller::NoWait(ElementPollerNoWait)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_poller_with_timeout() {
        let mut poller =
            ElementPollerWithTimeout::new(Duration::from_secs(1), Duration::from_millis(500));
        assert!(poller.tick().await);
        // This should have waited 500ms already.
        // Waiting an additional 500ms should exceed the timeout.
        sleep(Duration::from_millis(500)).await;
        assert!(!poller.tick().await);
    }

    #[tokio::test]
    async fn test_poller_nowait() {
        let mut poller = ElementPollerNoWait;
        assert!(!poller.tick().await); // Should instantly return false.
    }
}
