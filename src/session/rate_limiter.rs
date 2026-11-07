use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::Mutex as Queue;
use tokio::time::{Instant, sleep};

use super::no_updates::has;
use crate::errors::{Error, Result};

/// What a request counts against. Each category has a bucket of its own, and every request also takes a token from
/// the `Global` one.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RateCategory {
    /// Sending, editing and forwarding text.
    Message,
    /// The same for photos, videos, documents and the like.
    Media,
    /// Reads and searches, and anything not named below.
    Query,
    /// Deleting, banning, pinning and other changes to a chat.
    Admin,
    /// Fetching updates and pinging.
    Bulk,
    /// Everything under `account.`.
    Account,
    /// Every request, whatever its category.
    Global,
}

const CATEGORIES: usize = 7;

impl RateCategory {
    /// The category of a request by its TL name, such as `messages.sendMessage`.
    ///
    /// The rules read the name as `messages.SendMessage` and look for words in it, the first match winning:
    /// `account.` ones are `Account`; `Send`, `Upload`, `Edit`, `Forward` or `Save` make `Media` with a media word
    /// in the name and `Message` without; `GetDifference`, `GetState`, `Ping` or `HttpWait` make `Bulk`, which comes
    /// before `Admin` so that `Ping` isn't taken for a `Pin`; `Delete`, `Ban`, `Kick`, `Unban`, `Promote`, `Toggle`,
    /// `Set`, `Pin`, `Report` or `Block` make `Admin`; the rest is `Query`.
    pub fn of(name: &str) -> RateCategory {
        let (namespace, method) = name.split_once('.').unwrap_or(("", name));
        if namespace == "account" {
            return RateCategory::Account;
        }
        let any = |words: &[&str]| words.iter().any(|word| has(method, word));
        if any(&["Send", "Upload", "Edit", "Forward", "Save"]) {
            let media = ["Media", "Photo", "Video", "Audio", "Document", "Animation", "Voice", "Sticker", "Round"];
            return if any(&media) { RateCategory::Media } else { RateCategory::Message };
        }
        if any(&["GetDifference", "GetState", "Ping", "HttpWait"]) {
            return RateCategory::Bulk;
        }
        if any(&["Delete", "Ban", "Kick", "Unban", "Promote", "Toggle", "Set", "Pin", "Report", "Block"]) {
            return RateCategory::Admin;
        }
        RateCategory::Query
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A rate in requests per second and a burst: how many can go at once after a quiet time.
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimits {
    limits: [(f64, f64); CATEGORIES],
}

impl Default for RateLimits {
    fn default() -> Self {
        RateLimits {
            limits: [(20.0, 30.0), (5.0, 10.0), (30.0, 50.0), (15.0, 20.0), (3.0, 5.0), (10.0, 15.0), (30.0, 40.0)],
        }
    }
}

impl RateLimits {
    /// Changes one category. The rate must be above 0 and finite, and the burst at least 1; `ClientBuilder` refuses
    /// anything else when it connects.
    #[must_use]
    pub fn limit(mut self, category: RateCategory, rate: f64, burst: f64) -> Self {
        self.limits[category.index()] = (rate, burst);
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        self.limits.iter().try_for_each(|&(rate, burst)| check(rate, burst))
    }
}

fn check(rate: f64, burst: f64) -> Result<()> {
    if !rate.is_finite() || rate <= 0.0 {
        return Err(Error::InvalidArgument("a rate limit must be a finite number above 0"));
    }
    if !burst.is_finite() || burst < 1.0 {
        return Err(Error::InvalidArgument("a rate limit's burst must be at least 1"));
    }
    Ok(())
}

struct State {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: Instant,
}

impl State {
    fn refill(&mut self, now: Instant) {
        let seconds = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + seconds * self.rate).min(self.burst);
        self.last = now;
    }
}

// A token bucket. The queue hands out turns in the order callers came and is held while its owner sleeps for a
// token, so a long wait doesn't wake everyone behind it at once.
struct Bucket {
    state: Mutex<State>,
    queue: Queue<()>,
}

impl Bucket {
    fn new((rate, burst): (f64, f64)) -> Bucket {
        let state = State { tokens: burst, rate, burst, last: Instant::now() };
        Bucket { state: Mutex::new(state), queue: Queue::new(()) }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        // Nothing under this lock can panic, but a poisoned bucket is still a usable one.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Takes a token if there is one, and says how long until there will be otherwise.
    fn try_take(&self) -> Result<(), Duration> {
        let mut state = self.state();
        state.refill(Instant::now());
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            return Ok(());
        }
        // A rate far below one a second can put the next token past what a Duration holds: that's never.
        let wait = Duration::try_from_secs_f64((1.0 - state.tokens) / state.rate).unwrap_or(Duration::MAX);
        Err(wait.max(Duration::from_micros(1)))
    }

    async fn take(&self) {
        let _turn = self.queue.lock().await;
        while let Err(wait) = self.try_take() {
            sleep(wait).await;
        }
    }

    fn give_back(&self) {
        let mut state = self.state();
        state.tokens = (state.tokens + 1.0).min(state.burst);
    }

    fn tokens(&self) -> f64 {
        let mut state = self.state();
        state.refill(Instant::now());
        state.tokens
    }
}

// A token taken from the category that goes back if the caller is dropped while it waits for the global one.
struct Refund<'a>(Option<&'a Bucket>);

impl Drop for Refund<'_> {
    fn drop(&mut self) {
        if let Some(bucket) = self.0 {
            bucket.give_back();
        }
    }
}

/// Spaces requests out on the client's side so Telegram doesn't answer FLOOD_WAIT. Waiters are served in the order
/// they came; dropping a waiting call takes nothing. Turn it on with `ClientBuilder::rate_limits`.
pub struct RateLimiter {
    buckets: [Bucket; CATEGORIES],
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter").field("congestion", &self.congestion()).finish()
    }
}

impl RateLimiter {
    pub(crate) fn new(limits: &RateLimits) -> RateLimiter {
        RateLimiter { buckets: limits.limits.map(Bucket::new) }
    }

    fn bucket(&self, category: RateCategory) -> &Bucket {
        &self.buckets[category.index()]
    }

    /// Waits for a token of `category` and one of the global bucket.
    pub async fn acquire(&self, category: RateCategory) {
        let own = self.bucket(category);
        own.take().await;
        if category == RateCategory::Global {
            return;
        }
        let mut refund = Refund(Some(own));
        self.bucket(RateCategory::Global).take().await;
        refund.0 = None;
    }

    /// Takes the tokens if both are there now; otherwise takes nothing and says so.
    pub fn try_acquire(&self, category: RateCategory) -> bool {
        let own = self.bucket(category);
        if own.try_take().is_err() {
            return false;
        }
        if category == RateCategory::Global || self.bucket(RateCategory::Global).try_take().is_ok() {
            return true;
        }
        own.give_back();
        false
    }

    /// How many requests of `category` could go right now, counting the fraction of a token that's built up.
    pub fn available(&self, category: RateCategory) -> f64 {
        self.bucket(category).tokens()
    }

    /// How used up the global bucket is, from 0 (quiet) to 1 (empty).
    pub fn congestion(&self) -> f64 {
        let global = self.bucket(RateCategory::Global);
        let burst = global.state().burst;
        1.0 - global.tokens() / burst
    }

    /// Changes one category's rate and burst for the calls that come next. The tokens it holds stay, up to the new
    /// burst.
    pub fn update_limits(&self, category: RateCategory, rate: f64, burst: f64) -> Result<()> {
        check(rate, burst)?;
        let mut state = self.bucket(category).state();
        state.refill(Instant::now());
        (state.rate, state.burst) = (rate, burst);
        state.tokens = state.tokens.min(burst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn limiter(category: RateCategory, rate: f64, burst: f64) -> RateLimiter {
        // The global bucket stays out of the way unless a test is about it.
        let limits = RateLimits::default().limit(RateCategory::Global, 1000.0, 1000.0).limit(category, rate, burst);
        RateLimiter::new(&limits)
    }

    #[tokio::test(start_paused = true)]
    async fn bursts_then_paces() {
        let limiter = limiter(RateCategory::Message, 2.0, 3.0);
        let start = Instant::now();
        for _ in 0..3 {
            limiter.acquire(RateCategory::Message).await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO, "the burst goes at once");
        for n in 1..=4 {
            limiter.acquire(RateCategory::Message).await;
            let expected = Duration::from_millis(500 * n);
            assert!(start.elapsed() >= expected && start.elapsed() < expected + Duration::from_millis(10), "{n}");
        }
        // Another category's bucket isn't touched.
        assert_eq!(limiter.available(RateCategory::Query), 50.0);
    }

    #[tokio::test(start_paused = true)]
    async fn waiters_are_served_in_order() {
        let limiter = Arc::new(limiter(RateCategory::Query, 1.0, 1.0));
        limiter.acquire(RateCategory::Query).await;
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for id in 0..4 {
            let (limiter, order) = (limiter.clone(), order.clone());
            tasks.push(tokio::spawn(async move {
                limiter.acquire(RateCategory::Query).await;
                order.lock().unwrap().push(id);
            }));
            // Let it reach the queue before the next one starts.
            tokio::task::yield_now().await;
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(*order.lock().unwrap(), [0, 1, 2, 3]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_acquire_consumes_nothing() {
        // The category has tokens but the global bucket is empty, so the call waits on the global one.
        let limits =
            RateLimits::default().limit(RateCategory::Global, 1.0, 1.0).limit(RateCategory::Message, 1.0, 30.0);
        let limiter = RateLimiter::new(&limits);
        limiter.acquire(RateCategory::Global).await;
        let waited = tokio::time::timeout(Duration::from_millis(100), limiter.acquire(RateCategory::Message)).await;
        assert!(waited.is_err());
        assert_eq!(limiter.available(RateCategory::Message), 30.0, "the category's token wasn't given back");
        assert!(!limiter.try_acquire(RateCategory::Message));
        assert_eq!(limiter.available(RateCategory::Message), 30.0, "try_acquire left a token behind");

        // And a call dropped while it waits in line for its category costs the global bucket nothing.
        let limits =
            RateLimits::default().limit(RateCategory::Global, 1000.0, 1000.0).limit(RateCategory::Admin, 1.0, 1.0);
        let busy = RateLimiter::new(&limits);
        busy.acquire(RateCategory::Admin).await;
        let waited = tokio::time::timeout(Duration::from_millis(100), busy.acquire(RateCategory::Admin)).await;
        assert!(waited.is_err());
        assert_eq!(busy.available(RateCategory::Global), 1000.0);
    }

    #[tokio::test(start_paused = true)]
    async fn limits_can_change_and_congestion_tracks_the_global_bucket() {
        let limiter = limiter(RateCategory::Bulk, 1.0, 1.0);
        assert_eq!(limiter.congestion(), 0.0);
        limiter.update_limits(RateCategory::Global, 10.0, 10.0).unwrap();
        for _ in 0..5 {
            limiter.acquire(RateCategory::Bulk).await;
            tokio::time::advance(Duration::from_secs(1)).await;
        }
        assert!(limiter.congestion() < 0.01);
        assert!(limiter.update_limits(RateCategory::Bulk, 0.0, 1.0).is_err());
        assert!(limiter.update_limits(RateCategory::Bulk, f64::NAN, 1.0).is_err());
        assert!(limiter.update_limits(RateCategory::Bulk, 1.0, 0.5).is_err());
        assert!(RateLimits::default().limit(RateCategory::Query, f64::INFINITY, 1.0).validate().is_err());
        assert!(RateLimits::default().validate().is_ok());
    }

    #[test]
    fn classification_table() {
        use RateCategory::*;
        let cases = [
            ("messages.sendMessage", Message),
            ("messages.editMessage", Message),
            ("messages.forwardMessages", Message),
            ("messages.sendMedia", Media),
            ("messages.sendMultiMedia", Media),
            ("photos.uploadProfilePhoto", Media),
            ("upload.saveFilePart", Message),
            ("messages.deleteMessages", Admin),
            ("channels.editBanned", Message),
            ("channels.toggleSlowMode", Admin),
            ("messages.setTyping", Admin),
            ("messages.updatePinnedMessage", Admin),
            ("account.updateProfile", Account),
            ("account.sendVerifyEmailCode", Account),
            ("updates.getDifference", Bulk),
            ("updates.getState", Bulk),
            ("ping", Bulk),
            ("messages.getHistory", Query),
            ("messages.search", Query),
            ("help.getConfig", Query),
            ("contacts.resolveUsername", Query),
        ];
        for (name, expected) in cases {
            assert_eq!(RateCategory::of(name), expected, "{name}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_tiny_rate_waits_instead_of_panicking() {
        let limits = RateLimits::default().limit(RateCategory::Query, 1e-30, 1.0);
        assert!(limits.validate().is_ok());
        let limiter = limiter(RateCategory::Query, 1e-30, 1.0);
        assert!(limiter.try_acquire(RateCategory::Query));
        assert!(!limiter.try_acquire(RateCategory::Query));
        let next = tokio::time::timeout(Duration::from_secs(3600), limiter.acquire(RateCategory::Query)).await;
        assert!(next.is_err(), "the second token came");
    }
}
