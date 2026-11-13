use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use crate::errors::Error;

/// Decides whether a request that failed is sent again, and after how long. It's asked after every failure of
/// `Client::invoke` and of file parts; what it answers with `ControlFlow::Continue(wait)` is slept, and the request
/// goes out again. Lost connections need no policy: they are reconnected and the waiting requests sent again on
/// their own. When it hands an error over, a file part still goes again a few times on a timeout (on a fresh
/// connection), and one not from a CDN on a server error (a 500 or a negative code, after a pause that doubles from
/// a second), so one such error doesn't fail a whole transfer.
///
/// The policy alone bounds the retries it asks for: nothing else stops a request that keeps failing, so a policy
/// that always answers `Continue` retries it for ever. A retried FLOOD_WAIT doesn't take new tokens from the
/// client's rate limiter, which counted the request once.
///
/// `Error::Timeout` and `Error::Disconnected` don't say whether Telegram ran the request: it may have, and only
/// the answer was lost. A retry sends the same bytes, so a request with a `random_id` (sending a message) is
/// recognised and not run twice; one without may be.
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
    /// Whether the request is one part of an upload or download: thousands go out for one big file, and one that
    /// gives up fails the whole transfer. Not for a part from a CDN, which the file's own DC serves when it fails.
    pub file_part: bool,
}

// AutoSleep sleeps off this many times its `tries` for one file part: gogram's 20 tries a part, with the default 5.
const PART_TRIES_PER_TRY: usize = 4;

/// Sleeps off FLOOD_WAITs of up to `threshold`, at most `tries` times per request (four times that for a file part,
/// one of thousands in a transfer that fails with it), and hands everything else to the caller. This is what a
/// client does unless it's given another policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoSleep {
    /// The longest wait slept off; longer ones go to the caller. `Duration::ZERO` hands every one over.
    pub threshold: Duration,
    /// How many waits one request sleeps off, four times that for a file part; 0 hands every one over.
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

    /// Sets how many waits one request sleeps off, four times that for a file part; 0 hands every one over.
    #[must_use]
    pub fn tries(mut self, tries: usize) -> Self {
        self.tries = tries;
        self
    }
}

impl RetryPolicy for AutoSleep {
    fn should_retry(&self, ctx: &RetryContext<'_>) -> ControlFlow<(), Duration> {
        let tries = if ctx.file_part { self.tries.saturating_mul(PART_TRIES_PER_TRY) } else { self.tries };
        match ctx.flood_wait {
            Some(wait) if wait <= self.threshold && ctx.fail_count as usize <= tries => ControlFlow::Continue(wait),
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
        ask_about(policy, flood_wait, fail_count, false)
    }

    fn ask_about(
        policy: &impl RetryPolicy,
        flood_wait: Option<u64>,
        fail_count: u32,
        file_part: bool,
    ) -> ControlFlow<(), Duration> {
        let error = Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_1"));
        let flood_wait = flood_wait.map(Duration::from_secs);
        let slept_so_far = Duration::ZERO;
        let ctx = RetryContext { request: "x.y", error: &error, fail_count, slept_so_far, flood_wait, file_part };
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

    #[test]
    fn auto_sleep_gives_a_file_part_four_times_the_tries() {
        let policy = AutoSleep::new();
        assert_eq!(ask_about(&policy, Some(3), 6, false), ControlFlow::Break(()), "a 6th wait for a request");
        assert_eq!(ask_about(&policy, Some(3), 20, true), ControlFlow::Continue(Duration::from_secs(3)));
        assert_eq!(ask_about(&policy, Some(3), 21, true), ControlFlow::Break(()), "a 21st wait is over the tries");
        assert_eq!(ask_about(&policy, Some(61), 1, true), ControlFlow::Break(()), "a wait over the threshold");
        assert_eq!(ask_about(&AutoSleep::new().tries(0), Some(1), 1, true), ControlFlow::Break(()));
    }
}
