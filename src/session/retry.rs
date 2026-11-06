use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use crate::errors::Error;

/// Decides whether a request that failed is sent again, and after how long. It's asked after every failure of
/// `Client::invoke` and of file parts; what it answers with `ControlFlow::Continue(wait)` is slept, and the request
/// goes out again. Lost connections need no policy: they are reconnected and the waiting requests sent again on
/// their own.
///
/// The policy alone bounds the retries: nothing else stops a request that keeps failing, so a policy that always
/// answers `Continue` retries it for ever. A retried FLOOD_WAIT doesn't take new tokens from the client's rate
/// limiter, which counted the request once.
pub trait RetryPolicy: Send + Sync + 'static {
    /// `Continue(wait)` sleeps `wait` and sends the request again; `Break(())` hands the error to the caller.
    fn should_retry(&self, ctx: &RetryContext<'_>) -> ControlFlow<(), Duration>;
}

/// What a [`RetryPolicy`] is told about a failure.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct RetryContext<'a> {
    /// The TL name of the request, such as `messages.sendMessage`.
    pub request: &'static str,
    /// The error the request failed with.
    pub error: &'a Error,
    /// How many times the policy has been asked about this request, this time included: 1 after the first failure.
    pub fail_count: u32,
    /// How long the request has been slept so far, over the retries the policy asked for.
    pub slept_so_far: Duration,
    /// How long Telegram says to wait, if the error is a FLOOD_WAIT or something like it, at least a second. File
    /// parts from a CDN get at most a minute of it.
    pub flood_wait: Option<Duration>,
}

/// Sleeps off FLOOD_WAITs of up to `threshold`, at most `tries` times per request, and hands everything else to the
/// caller. This is what a client does unless it's given another policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoSleep {
    /// The longest wait slept off; longer ones go to the caller. `Duration::ZERO` hands every one over.
    pub threshold: Duration,
    /// How many waits one request sleeps off; 0 hands every one over.
    pub tries: usize,
}

impl Default for AutoSleep {
    fn default() -> Self {
        AutoSleep { threshold: Duration::from_secs(60), tries: 5 }
    }
}

impl AutoSleep {
    /// Same as `Default`: sleeps off waits of up to a minute, five times per request.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the longest wait slept off; longer ones go to the caller. `Duration::ZERO` hands every one over.
    #[must_use]
    pub fn threshold(mut self, threshold: Duration) -> Self {
        self.threshold = threshold;
        self
    }

    /// Sets how many waits one request sleeps off; 0 hands every one over.
    #[must_use]
    pub fn tries(mut self, tries: usize) -> Self {
        self.tries = tries;
        self
    }
}

impl RetryPolicy for AutoSleep {
    fn should_retry(&self, ctx: &RetryContext<'_>) -> ControlFlow<(), Duration> {
        match ctx.flood_wait {
            Some(wait) if wait <= self.threshold && ctx.fail_count as usize <= self.tries => {
                ControlFlow::Continue(wait)
            }
            _ => ControlFlow::Break(()),
        }
    }
}

/// Never retries: every failure, FLOOD_WAITs included, goes to the caller.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoRetries;

impl RetryPolicy for NoRetries {
    fn should_retry(&self, _: &RetryContext<'_>) -> ControlFlow<(), Duration> {
        ControlFlow::Break(())
    }
}

/// A client's retry policy, as `Options` holds it.
#[derive(Clone)]
pub struct Retry(pub Arc<dyn RetryPolicy>);

impl Retry {
    /// Wraps `policy` to be shared by every request of a client.
    pub fn new(policy: impl RetryPolicy) -> Retry {
        Retry(Arc::new(policy))
    }
}

impl Default for Retry {
    fn default() -> Self {
        Retry::new(AutoSleep::default())
    }
}

impl std::fmt::Debug for Retry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Retry")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::RpcError;

    fn ask(policy: &impl RetryPolicy, flood_wait: Option<u64>, fail_count: u32) -> ControlFlow<(), Duration> {
        let error = Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_1"));
        let flood_wait = flood_wait.map(Duration::from_secs);
        let ctx = RetryContext { request: "x.y", error: &error, fail_count, slept_so_far: Duration::ZERO, flood_wait };
        policy.should_retry(&ctx)
    }

    #[test]
    fn auto_sleep_follows_its_threshold_and_tries() {
        let policy = AutoSleep { threshold: Duration::from_secs(10), tries: 2 };
        let sleep = |secs| ControlFlow::Continue(Duration::from_secs(secs));
        assert_eq!(ask(&policy, Some(10), 1), sleep(10));
        assert_eq!(ask(&policy, Some(3), 2), sleep(3));
        assert_eq!(ask(&policy, Some(3), 3), ControlFlow::Break(()), "a third retry is over the tries");
        assert_eq!(ask(&policy, Some(11), 1), ControlFlow::Break(()), "a wait over the threshold");
        assert_eq!(ask(&policy, None, 1), ControlFlow::Break(()), "not a flood wait");
        assert_eq!(ask(&AutoSleep { tries: 0, ..policy }, Some(1), 1), ControlFlow::Break(()));
        assert_eq!(ask(&AutoSleep { threshold: Duration::ZERO, ..policy }, Some(1), 1), ControlFlow::Break(()));
        assert_eq!(AutoSleep::default(), AutoSleep { threshold: Duration::from_secs(60), tries: 5 });
    }

    #[test]
    fn no_retries_never_retries() {
        assert_eq!(ask(&NoRetries, Some(1), 1), ControlFlow::Break(()));
    }

    #[test]
    fn auto_sleep_new_keeps_the_defaults_a_setter_leaves_alone() {
        let patient = AutoSleep::new().threshold(Duration::from_secs(300));
        assert_eq!(patient.tries, 5);
        assert_eq!(ask(&patient, Some(200), 1), ControlFlow::Continue(Duration::from_secs(200)));
        assert_eq!(ask(&AutoSleep::new().tries(0), Some(1), 1), ControlFlow::Break(()));
    }
}
