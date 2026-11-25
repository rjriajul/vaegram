use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::{Mutex as Queue, MutexGuard as Turn};
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
const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const MINUTE: Duration = Duration::from_secs(60);
// As far ahead as tokio's own far-off instant: an `Instant` doesn't reach much further.
const MAX_HOLD: Duration = Duration::from_secs(30 * 365 * 24 * 60 * 60);

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

/// A rate in requests per second and a burst for each category: how many can go at once after a quiet time; and,
/// optionally, a cap on how many go in any 24 hours, and limits for each chat ([`per_chat`](Self::per_chat)).
/// [`Default`] is the set for bots, at Bot API speeds (20 messages a second), with no limits per chat;
/// [`user_account`](Self::user_account) is the one a user account gets unless the client is given one.
#[derive(Clone, Debug, PartialEq)]
pub struct RateLimits {
    limits: [(f64, f64); CATEGORIES],
    daily: [Option<u32>; CATEGORIES],
    per_chat: Option<ChatRateLimits>,
}

impl Default for RateLimits {
    fn default() -> Self {
        RateLimits {
            limits: [(20.0, 30.0), (5.0, 10.0), (30.0, 50.0), (15.0, 20.0), (3.0, 5.0), (10.0, 15.0), (30.0, 40.0)],
            daily: [None; CATEGORIES],
            per_chat: None,
        }
    }
}

impl RateLimits {
    /// What a user account gets when `ClientBuilder::rate_limits` isn't set, in requests per second and burst:
    ///
    /// | Category  | Rate | Burst |
    /// |-----------|------|-------|
    /// | `Message` | 1    | 5     |
    /// | `Media`   | 0.5  | 3     |
    /// | `Query`   | 5    | 20    |
    /// | `Admin`   | 1    | 5     |
    /// | `Bulk`    | 3    | 5     |
    /// | `Account` | 0.5  | 3     |
    /// | `Global`  | 8    | 30    |
    ///
    /// Telegram publishes no limits for user accounts, and one that sends like a bot risks a ban, not just a
    /// FLOOD_WAIT. So what changes things goes at about a person's pace in an official app: a message or an edit a
    /// second, a file every two seconds, a chat change a second, an account change every two seconds, each with a
    /// small burst. Reads go faster, as an app reads in bursts when it opens, and the update fetches the client makes
    /// itself keep the bots' pace. File parts never wait here. No daily cap; add one with
    /// [`daily_limit`](Self::daily_limit). Each chat also gets [`ChatRateLimits::default`]: a message a second, and
    /// in a group at most 20 a minute; [`no_per_chat`](Self::no_per_chat) takes them off.
    pub fn user_account() -> RateLimits {
        RateLimits {
            limits: [(1.0, 5.0), (0.5, 3.0), (5.0, 20.0), (1.0, 5.0), (3.0, 5.0), (0.5, 3.0), (8.0, 30.0)],
            daily: [None; CATEGORIES],
            per_chat: Some(ChatRateLimits::default()),
        }
    }

    /// Changes one category. The rate must be above 0 and finite, and the burst at least 1; `ClientBuilder` refuses
    /// anything else when it connects.
    #[must_use]
    pub fn limit(mut self, category: RateCategory, rate: f64, burst: f64) -> Self {
        self.limits[category.index()] = (rate, burst);
        self
    }

    /// Caps `category` at `max` requests in any 24 hours, on top of its rate: once `max` have gone, the next waits
    /// until the oldest of them is a day old. Off by default. The count starts with the client, so a restart forgets
    /// it, and the limiter keeps the time of each request of the last day. `max` must be at least 1;
    /// `ClientBuilder` refuses 0 when it connects.
    #[must_use]
    pub fn daily_limit(mut self, category: RateCategory, max: u32) -> Self {
        self.daily[category.index()] = Some(max);
        self
    }

    /// Limits what goes to each chat as well, on top of the categories; see [`ChatRateLimits`]. Off in
    /// [`Default`], on in [`user_account`](Self::user_account). For a bot that sends to many chats:
    /// `RateLimits::default().per_chat(ChatRateLimits::default())`.
    #[must_use]
    pub fn per_chat(mut self, limits: ChatRateLimits) -> Self {
        self.per_chat = Some(limits);
        self
    }

    /// Takes the limits for each chat off, leaving the categories'.
    #[must_use]
    pub fn no_per_chat(mut self) -> Self {
        self.per_chat = None;
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        self.limits.iter().try_for_each(|&(rate, burst)| check(rate, burst))?;
        if self.daily.contains(&Some(0)) {
            return Err(Error::InvalidArgument("a daily limit must be at least 1"));
        }
        self.per_chat.as_ref().map_or(Ok(()), ChatRateLimits::validate)
    }
}

/// Limits on what goes to any one chat, on top of the categories, since Telegram answers FLOOD_WAIT to more than
/// about one message a second in a chat, or 20 a minute in a group. The limiter keeps a bucket for each chat by the
/// chat's id, so `@name` and the id it resolves to share one, and a chat that waits never holds up another.
///
/// [`Default`] gives a private chat (a positive id) a message a second, with no burst; a group, supergroup or channel
/// (a negative id) the same and at most 20 messages in any 60 seconds. It keeps up to 10 000 chats. When a new chat
/// comes and there's no room, the chats that have had nothing for 10 minutes and whose buckets have filled up again
/// go first, then the one used longest ago, and a message to it later starts from a full bucket. A bucket a call is
/// using never goes: while every one is, the limiter keeps more chats than the cap.
///
/// The client counts a call against them when the call sends, edits, forwards, copies or pins messages in one chat,
/// and for [`Client::invoke_for_chat`](crate::Client::invoke_for_chat). A FLOOD_WAIT or SLOWMODE_WAIT answer that a
/// call sleeps off holds that chat's next message back by the wait, and no other chat's. Chat actions and drafts
/// don't count: they aren't messages, and a typing action before each message would halve what a chat can get.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatRateLimits {
    private: (f64, f64),
    group: (f64, f64),
    group_per_minute: Option<u32>,
    max_tracked_chats: usize,
    idle_ttl: Duration,
}

impl Default for ChatRateLimits {
    fn default() -> Self {
        ChatRateLimits {
            private: (1.0, 1.0),
            group: (1.0, 1.0),
            group_per_minute: Some(20),
            max_tracked_chats: 10_000,
            idle_ttl: Duration::from_secs(10 * 60),
        }
    }
}

impl ChatRateLimits {
    /// Changes the rate in messages per second and the burst of a private chat, one with a positive id. The rate
    /// must be above 0 and finite, and the burst at least 1; `ClientBuilder` refuses anything else when it connects.
    #[must_use]
    pub fn private(mut self, rate: f64, burst: f64) -> Self {
        self.private = (rate, burst);
        self
    }

    /// Changes the rate and burst of a group, supergroup or channel, a chat with a negative id, as
    /// [`private`](Self::private) does for a private chat.
    #[must_use]
    pub fn group(mut self, rate: f64, burst: f64) -> Self {
        self.group = (rate, burst);
        self
    }

    /// Caps a group, supergroup or channel at `max` messages in any 60 seconds, on top of its rate: once `max` have
    /// gone, the next waits until the oldest of them is a minute old. `None` takes the cap off. `max` must be at
    /// least 1.
    #[must_use]
    pub fn group_per_minute(mut self, max: Option<u32>) -> Self {
        self.group_per_minute = max;
        self
    }

    /// How many chats the limiter keeps a bucket for, at least 1. When a new chat comes and there's no room, the
    /// idle buckets go first ([`idle_ttl`](Self::idle_ttl)), then the one used longest ago that no call is using.
    #[must_use]
    pub fn max_tracked_chats(mut self, max: usize) -> Self {
        self.max_tracked_chats = max;
        self
    }

    /// How long a chat has nothing before its bucket, once full again, counts as idle: the first to go when a new
    /// chat needs room. Buckets aren't dropped on a timer, only to make room.
    #[must_use]
    pub fn idle_ttl(mut self, ttl: Duration) -> Self {
        self.idle_ttl = ttl;
        self
    }

    fn validate(&self) -> Result<()> {
        check(self.private.0, self.private.1)?;
        check(self.group.0, self.group.1)?;
        if self.group_per_minute == Some(0) {
            return Err(Error::InvalidArgument("a group's per-minute cap must be at least 1"));
        }
        if self.max_tracked_chats == 0 {
            return Err(Error::InvalidArgument("the rate limiter must track at least 1 chat"));
        }
        Ok(())
    }

    /// A fresh bucket for `chat_id`. Chat ids are Bot API style: users positive, groups and channels negative.
    fn bucket(&self, chat_id: i64) -> Bucket {
        if chat_id < 0 {
            Bucket::new(self.group, self.group_per_minute.map(|max| (MINUTE, max)))
        } else {
            Bucket::new(self.private, None)
        }
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

/// A cap on requests in any `span`, such as a day: when each of the last span's went, oldest first.
struct Window {
    span: Duration,
    max: usize,
    sent: VecDeque<Instant>,
}

impl Window {
    /// How long until another request fits under the cap: zero when one does now.
    fn wait(&mut self, now: Instant) -> Duration {
        while self.sent.front().is_some_and(|&at| now.saturating_duration_since(at) >= self.span) {
            self.sent.pop_front();
        }
        match self.sent.front() {
            Some(&oldest) if self.sent.len() >= self.max => {
                self.span.saturating_sub(now.saturating_duration_since(oldest))
            }
            _ => Duration::ZERO,
        }
    }

    /// How many more may go now.
    fn left(&mut self, now: Instant) -> usize {
        self.wait(now);
        self.max.saturating_sub(self.sent.len())
    }
}

struct State {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: Instant,
    window: Option<Window>,
}

impl State {
    fn refill(&mut self, now: Instant) {
        // A held bucket has `last` ahead of now, and nothing builds up before it.
        if now <= self.last {
            return;
        }
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
    fn new((rate, burst): (f64, f64), window: Option<(Duration, u32)>) -> Bucket {
        let window = window.map(|(span, max)| Window { span, max: max as usize, sent: VecDeque::new() });
        let state = State { tokens: burst, rate, burst, last: Instant::now(), window };
        Bucket { state: Mutex::new(state), queue: Queue::new(()) }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        // Nothing under this lock can panic, but a poisoned bucket is still a usable one.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a token is there, the bucket isn't held and its cap has room, taking the token if `take` says so; if
    /// not, how long until all three will be.
    fn token(&self, take: bool) -> Result<(), Duration> {
        let mut state = self.state();
        let now = Instant::now();
        state.refill(now);
        let cap = state.window.as_mut().map_or(Duration::ZERO, |window| window.wait(now));
        let held = state.last.saturating_duration_since(now);
        if state.tokens >= 1.0 && cap.is_zero() && held.is_zero() {
            if take {
                state.tokens -= 1.0;
                if let Some(window) = &mut state.window {
                    window.sent.push_back(now);
                }
            }
            return Ok(());
        }
        // A rate far below one a second can put the next token past what a Duration holds: that's never.
        let token = Duration::try_from_secs_f64((1.0 - state.tokens).max(0.0) / state.rate).unwrap_or(Duration::MAX);
        Err(token.saturating_add(held).max(cap).max(Duration::from_micros(1)))
    }

    /// Takes a token if there is one, the bucket isn't held and its cap has room, and says how long until all three
    /// will otherwise.
    fn try_take(&self) -> Result<(), Duration> {
        self.token(true)
    }

    async fn take(&self) {
        let _turn = self.queue.lock().await;
        self.take_in_turn().await;
    }

    /// Waits for the bucket's turn and for a token, but takes none: the caller takes it with `take_in_turn` while it
    /// holds the turn, once it is about to go.
    async fn ready(&self) -> Turn<'_, ()> {
        let turn = self.queue.lock().await;
        while let Err(wait) = self.token(false) {
            sleep(wait).await;
        }
        turn
    }

    /// Takes a token, waiting if it isn't there, for the caller that holds the bucket's turn.
    async fn take_in_turn(&self) {
        while let Err(wait) = self.try_take() {
            sleep(wait).await;
        }
    }

    fn give_back(&self) {
        let mut state = self.state();
        if let Some(window) = &mut state.window {
            window.sent.pop_back();
        }
        // A hold since the token was taken emptied the bucket, and the token doesn't come back past it.
        if state.last <= Instant::now() {
            state.tokens = (state.tokens + 1.0).min(state.burst);
        }
    }

    /// The tokens there are, counting the fraction that's built up, and no more than the cap has room for.
    fn tokens(&self) -> f64 {
        let mut state = self.state();
        let now = Instant::now();
        state.refill(now);
        let tokens = state.tokens;
        match &mut state.window {
            Some(window) => tokens.min(window.left(now) as f64),
            None => tokens,
        }
    }

    /// Empties the bucket and lets nothing build up before `until`, after a FLOOD_WAIT on what it limits. The call
    /// that met the wait goes again by itself when it ends, so the next one here waits a token's time more.
    fn hold_until(&self, until: Instant) {
        let mut state = self.state();
        state.tokens = 0.0;
        state.last = state.last.max(until);
    }

    /// Whether nothing touched the bucket for `ttl`, it has filled up since and its cap is clear, so that a new one
    /// would be no different.
    fn rested(&self, now: Instant, ttl: Duration) -> bool {
        let state = self.state();
        let idle = now.saturating_duration_since(state.last);
        let full = state.tokens + idle.as_secs_f64() * state.rate >= state.burst;
        let clear = state
            .window
            .as_ref()
            .is_none_or(|window| window.sent.back().is_none_or(|&at| now.saturating_duration_since(at) >= window.span));
        idle >= ttl && full && clear
    }

    /// When the bucket was last touched, or when its hold ends.
    fn last(&self) -> Instant {
        self.state().last
    }
}

// The tokens a call took of its category and of the global bucket, which go back if it's dropped before it goes.
#[derive(Default)]
struct Refund<'a>([Option<&'a Bucket>; 2]);

impl<'a> Refund<'a> {
    fn push(&mut self, bucket: &'a Bucket) {
        if let Some(slot) = self.0.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(bucket);
        }
    }

    /// The call has every token it needs: none go back.
    fn keep(mut self) {
        self.0 = [None; 2];
    }
}

impl Drop for Refund<'_> {
    fn drop(&mut self) {
        for bucket in self.0.iter().flatten() {
            bucket.give_back();
        }
    }
}

// The buckets of the chats sent to lately, by chat id. The lock is held only to find or add one, never across a
// wait.
struct Chats {
    limits: ChatRateLimits,
    buckets: Mutex<HashMap<i64, Arc<Bucket>>>,
}

impl Chats {
    fn buckets(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Arc<Bucket>>> {
        // As with a bucket's state: nothing under this lock can panic.
        self.buckets.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `chat_id`'s bucket, made the first time. When there's no room for it, the idle ones go, and if that isn't
    /// enough, the one used longest ago. A bucket a call is using stays: while every one is, the map grows past
    /// `max_tracked_chats` until they're done.
    fn get(&self, chat_id: i64) -> Arc<Bucket> {
        let mut buckets = self.buckets();
        if let Some(bucket) = buckets.get(&chat_id) {
            return Arc::clone(bucket);
        }
        let max = self.limits.max_tracked_chats;
        if buckets.len() >= max {
            let (now, ttl) = (Instant::now(), self.limits.idle_ttl);
            // A bucket a call holds is in use, however long ago it was touched.
            buckets.retain(|_, bucket| Arc::strong_count(bucket) > 1 || !bucket.rested(now, ttl));
            while buckets.len() >= max {
                let free = buckets.iter().filter(|(_, bucket)| Arc::strong_count(bucket) == 1);
                let Some(oldest) = free.min_by_key(|(_, bucket)| bucket.last()).map(|(&id, _)| id) else { break };
                buckets.remove(&oldest);
            }
        }
        let bucket = Arc::new(self.limits.bucket(chat_id));
        buckets.insert(chat_id, Arc::clone(&bucket));
        bucket
    }
}

/// Spaces requests out on the client's side so Telegram doesn't answer FLOOD_WAIT. Waiters are served in the order
/// they came; dropping a waiting call takes nothing. A user account has one by default, with
/// [`RateLimits::user_account`], which stands aside for a bot and before login; `ClientBuilder::rate_limits` sets
/// others, for any account. With [`ChatRateLimits`] it also keeps a bucket for each chat calls go to.
pub struct RateLimiter {
    buckets: [Bucket; CATEGORIES],
    chats: Option<Chats>,
    // Set on the default one: it holds calls back only while `user` says the account is a user.
    users_only: bool,
    user: AtomicBool,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimiter").field("congestion", &self.congestion()).finish()
    }
}

impl RateLimiter {
    pub(crate) fn new(limits: &RateLimits) -> RateLimiter {
        let buckets = std::array::from_fn(|i| Bucket::new(limits.limits[i], limits.daily[i].map(|max| (DAY, max))));
        let chats = limits.per_chat.clone().map(|limits| Chats { limits, buckets: Mutex::default() });
        RateLimiter { buckets, chats, users_only: false, user: AtomicBool::new(false) }
    }

    /// The limiter of a client given none: [`RateLimits::user_account`], holding calls back only while the account
    /// is a user, which `user` says it is already or not.
    pub(crate) fn for_user_accounts(user: bool) -> RateLimiter {
        RateLimiter { users_only: true, user: AtomicBool::new(user), ..RateLimiter::new(&RateLimits::user_account()) }
    }

    /// Tells the limiter whether the account logged in is a user, for the default one to stand aside otherwise.
    pub(crate) fn set_user(&self, user: bool) {
        self.user.store(user, Ordering::Relaxed);
    }

    fn stands_aside(&self) -> bool {
        self.users_only && !self.user.load(Ordering::Relaxed)
    }

    fn bucket(&self, category: RateCategory) -> &Bucket {
        &self.buckets[category.index()]
    }

    /// `chat_id`'s bucket, when there is a chat and the limiter has limits for chats.
    fn chat(&self, chat_id: Option<i64>) -> Option<Arc<Bucket>> {
        Some(self.chats.as_ref()?.get(chat_id?))
    }

    /// The buckets a call takes a token of after its chat's turn came: its category's, then the global one, unless
    /// the category is the global one or `global` is off.
    fn route(&self, category: RateCategory, global: bool) -> impl Iterator<Item = &Bucket> {
        let global = (global && category != RateCategory::Global).then(|| self.bucket(RateCategory::Global));
        [Some(self.bucket(category)), global].into_iter().flatten()
    }

    /// Waits for a token of `category` and one of the global bucket.
    pub async fn acquire(&self, category: RateCategory) {
        self.acquire_routed(category, None, true).await;
    }

    /// Waits for `chat_id`'s turn, then for a token of `category` and of the global bucket, and takes the chat's
    /// token last, as the call goes. Without [`ChatRateLimits`] it is [`acquire`](Self::acquire). Chat ids are Bot
    /// API style: users positive, groups and channels negative.
    pub async fn acquire_chat(&self, category: RateCategory, chat_id: i64) {
        self.acquire_routed(category, Some(chat_id), true).await;
    }

    /// `acquire_chat` when there's a chat, `acquire` when not, past the global bucket when `global` is off. A token
    /// taken goes back if the call is dropped before it has them all.
    pub(crate) async fn acquire_routed(&self, category: RateCategory, chat_id: Option<i64>, global: bool) {
        if self.stands_aside() {
            return;
        }
        let chat = self.chat(chat_id);
        // The chat's turn is kept until the call goes, and its token taken then, so the next call to the chat is
        // spaced from when this one went. Waiting for the turn holds no token another chat could use.
        let _turn = match &chat {
            Some(chat) => Some(chat.ready().await),
            None => None,
        };
        let mut refund = Refund::default();
        for bucket in self.route(category, global) {
            bucket.take().await;
            refund.push(bucket);
        }
        if let Some(chat) = &chat {
            // Normally there at once; a penalty that came meanwhile is waited out here.
            chat.take_in_turn().await;
        }
        refund.keep();
    }

    /// Takes the tokens if both are there now; otherwise takes nothing and says so.
    pub fn try_acquire(&self, category: RateCategory) -> bool {
        self.try_acquire_routed(category, None)
    }

    /// [`try_acquire`](Self::try_acquire) that takes a token of `chat_id`'s bucket too.
    pub fn try_acquire_chat(&self, category: RateCategory, chat_id: i64) -> bool {
        self.try_acquire_routed(category, Some(chat_id))
    }

    fn try_acquire_routed(&self, category: RateCategory, chat_id: Option<i64>) -> bool {
        if self.stands_aside() {
            return true;
        }
        let chat = self.chat(chat_id);
        let mut refund = Refund::default();
        for bucket in self.route(category, true) {
            if bucket.try_take().is_err() {
                return false;
            }
            refund.push(bucket);
        }
        if chat.is_some_and(|chat| chat.try_take().is_err()) {
            return false;
        }
        refund.keep();
        true
    }

    /// Holds `chat_id`'s next call back until `wait` from now, and one token's time more, as after Telegram answered
    /// a call to it with FLOOD_WAIT or SLOWMODE_WAIT. Other chats and the categories aren't touched. The client does
    /// it by itself for the calls it sends to one chat. Does nothing without [`ChatRateLimits`].
    pub fn penalize_chat(&self, chat_id: i64, wait: Duration) {
        if self.stands_aside() {
            return;
        }
        if let Some(bucket) = self.chat(Some(chat_id)) {
            bucket.hold_until(Instant::now() + wait.min(MAX_HOLD));
        }
    }

    /// How many chats the limiter keeps a bucket for, at most
    /// [`max_tracked_chats`](ChatRateLimits::max_tracked_chats).
    pub fn tracked_chats(&self) -> usize {
        self.chats.as_ref().map_or(0, |chats| chats.buckets().len())
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

    #[tokio::test(start_paused = true)]
    async fn a_daily_cap_waits_for_the_oldest_request_to_turn_a_day_old() {
        let limits = RateLimits::default()
            .limit(RateCategory::Global, 1000.0, 1000.0)
            .limit(RateCategory::Message, 1000.0, 1000.0)
            .daily_limit(RateCategory::Message, 3);
        assert!(limits.validate().is_ok());
        let limiter = RateLimiter::new(&limits);
        let start = Instant::now();
        for _ in 0..3 {
            limiter.acquire(RateCategory::Message).await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO, "the cap isn't a rate");
        assert_eq!(limiter.available(RateCategory::Message), 0.0);
        assert!(!limiter.try_acquire(RateCategory::Message));
        let early = tokio::time::timeout(Duration::from_secs(3600), limiter.acquire(RateCategory::Message)).await;
        assert!(early.is_err(), "a fourth went within the day");
        limiter.acquire(RateCategory::Message).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= DAY && elapsed < DAY + Duration::from_millis(10), "{elapsed:?}");
        assert!(limiter.try_acquire(RateCategory::Query), "another category has no cap");
        assert!(RateLimits::default().daily_limit(RateCategory::Global, 0).validate().is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_acquire_leaves_the_daily_count_alone() {
        // The category's token is taken, then the call waits on the global bucket and is dropped there.
        let limits = RateLimits::default()
            .limit(RateCategory::Global, 1.0, 1.0)
            .limit(RateCategory::Message, 1000.0, 1000.0)
            .daily_limit(RateCategory::Message, 2);
        let limiter = RateLimiter::new(&limits);
        limiter.acquire(RateCategory::Global).await;
        let waited = tokio::time::timeout(Duration::from_millis(100), limiter.acquire(RateCategory::Message)).await;
        assert!(waited.is_err());
        assert_eq!(limiter.available(RateCategory::Message), 2.0);
    }

    #[tokio::test(start_paused = true)]
    async fn the_default_limiter_holds_back_only_a_user() {
        let limiter = RateLimiter::for_user_accounts(false);
        let start = Instant::now();
        for _ in 0..100 {
            limiter.acquire(RateCategory::Message).await;
        }
        assert!(limiter.try_acquire(RateCategory::Media));
        for _ in 0..2 {
            assert!(limiter.try_acquire_chat(RateCategory::Message, 7));
        }
        limiter.penalize_chat(7, Duration::from_secs(30));
        assert_eq!(limiter.tracked_chats(), 0, "a bot's chats were tracked");
        assert_eq!(start.elapsed(), Duration::ZERO, "a bot, or a client not logged in, was held back");

        limiter.set_user(true);
        for _ in 0..6 {
            limiter.acquire(RateCategory::Message).await;
        }
        // A burst of five, then a message a second.
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_secs(1) && elapsed < Duration::from_millis(1010), "{elapsed:?}");
        assert!(!limiter.try_acquire(RateCategory::Message));

        limiter.set_user(false);
        assert!(limiter.try_acquire(RateCategory::Message));
    }

    #[test]
    fn user_account_limits_are_valid_and_no_faster_than_the_bots() {
        let (user, bot) = (RateLimits::user_account(), RateLimits::default());
        assert!(user.validate().is_ok());
        for (u, b) in user.limits.iter().zip(&bot.limits) {
            assert!(u.0 <= b.0 && u.1 <= b.1, "{u:?} against {b:?}");
        }
        assert_eq!(user.limits[RateCategory::Message.index()], (1.0, 5.0));
        assert_eq!(user.per_chat, Some(ChatRateLimits::default()));
        assert_eq!(bot.per_chat, None);
        assert_eq!(user.no_per_chat().per_chat, None);
    }

    // Only the chats' buckets hold calls back.
    fn chat_limiter(chats: ChatRateLimits) -> RateLimiter {
        let limits = RateLimits::default()
            .limit(RateCategory::Global, 1000.0, 1000.0)
            .limit(RateCategory::Message, 1000.0, 1000.0)
            .per_chat(chats);
        RateLimiter::new(&limits)
    }

    fn near(elapsed: Duration, expected: Duration) -> bool {
        elapsed >= expected && elapsed < expected + Duration::from_millis(10)
    }

    #[tokio::test(start_paused = true)]
    async fn a_private_chat_gets_a_message_a_second() {
        let limiter = chat_limiter(ChatRateLimits::default());
        let start = Instant::now();
        for n in 0..4 {
            limiter.acquire_chat(RateCategory::Message, 7).await;
            let elapsed = start.elapsed();
            assert!(near(elapsed, Duration::from_secs(n)), "{n}: {elapsed:?}");
        }
        assert!(!limiter.try_acquire_chat(RateCategory::Message, 7));
        // A call that names no chat doesn't wait for one.
        assert!(limiter.try_acquire(RateCategory::Message));
    }

    #[tokio::test(start_paused = true)]
    async fn a_group_gets_twenty_messages_a_minute() {
        let limiter = chat_limiter(ChatRateLimits::default());
        let start = Instant::now();
        for _ in 0..20 {
            limiter.acquire_chat(RateCategory::Message, -5).await;
        }
        // A message a second, so the twentieth goes at 19 s; the next waits for the first to be a minute old.
        assert!(near(start.elapsed(), Duration::from_secs(19)), "{:?}", start.elapsed());
        limiter.acquire_chat(RateCategory::Message, -5).await;
        assert!(near(start.elapsed(), Duration::from_secs(60)), "{:?}", start.elapsed());
        limiter.acquire_chat(RateCategory::Message, -5).await;
        assert!(near(start.elapsed(), Duration::from_secs(61)), "{:?}", start.elapsed());

        // Without the cap a group goes at its rate alone.
        let uncapped = chat_limiter(ChatRateLimits::default().group_per_minute(None));
        let start = Instant::now();
        for _ in 0..21 {
            uncapped.acquire_chat(RateCategory::Message, -5).await;
        }
        assert!(near(start.elapsed(), Duration::from_secs(20)), "{:?}", start.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn a_dropped_chat_acquire_gives_its_tokens_back() {
        // The chat and the category refill slowly. The global bucket is empty, so the call gets its chat's turn,
        // takes the category's token, then waits on the global one.
        let limits = RateLimits::default()
            .limit(RateCategory::Global, 1.0, 1.0)
            .limit(RateCategory::Message, 0.001, 30.0)
            .per_chat(ChatRateLimits::default().private(0.001, 1.0));
        let limiter = RateLimiter::new(&limits);
        limiter.acquire(RateCategory::Global).await;
        let call = limiter.acquire_chat(RateCategory::Message, 7);
        assert!(tokio::time::timeout(Duration::from_millis(100), call).await.is_err());
        assert_eq!(limiter.available(RateCategory::Message), 30.0, "the category's token wasn't given back");
        tokio::time::advance(Duration::from_secs(1)).await;
        assert!(limiter.try_acquire_chat(RateCategory::Message, 7), "the dropped call used up the chat's token");

        // Dropped while it waits for its chat, a call costs the category nothing.
        let call = limiter.acquire_chat(RateCategory::Message, 7);
        assert!(tokio::time::timeout(Duration::from_millis(100), call).await.is_err());
        assert_eq!(limiter.available(RateCategory::Message).floor(), 29.0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_chat_is_spaced_from_when_its_calls_go() {
        // The global bucket holds the calls up, so each goes a while after its chat's turn came.
        let limits = RateLimits::default()
            .limit(RateCategory::Global, 1.0, 1.0)
            .limit(RateCategory::Message, 1000.0, 1000.0)
            .per_chat(ChatRateLimits::default().group(1000.0, 1000.0).group_per_minute(Some(2)));
        let limiter = Arc::new(RateLimiter::new(&limits));
        limiter.acquire(RateCategory::Global).await;
        let start = Instant::now();
        let calls: Vec<_> = (0..3)
            .map(|_| {
                let limiter = Arc::clone(&limiter);
                tokio::spawn(async move {
                    limiter.acquire_chat(RateCategory::Message, -5).await;
                    start.elapsed()
                })
            })
            .collect();
        let mut went = Vec::new();
        for call in calls {
            went.push(call.await.unwrap());
        }
        went.sort();
        // Two a minute: no 60 seconds hold all three.
        assert!(went[2] - went[0] >= Duration::from_secs(60), "{went:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn a_chat_that_waits_holds_up_no_other() {
        let limiter = Arc::new(chat_limiter(ChatRateLimits::default().private(0.001, 1.0).group(0.001, 1.0)));
        limiter.acquire_chat(RateCategory::Message, 7).await;
        let waiting = tokio::spawn({
            let limiter = Arc::clone(&limiter);
            async move { limiter.acquire_chat(RateCategory::Message, 7).await }
        });
        tokio::task::yield_now().await;
        let start = Instant::now();
        limiter.acquire_chat(RateCategory::Message, 8).await;
        limiter.acquire_chat(RateCategory::Message, -5).await;
        assert_eq!(start.elapsed(), Duration::ZERO, "another chat waited");
        assert!(!waiting.is_finished());
        waiting.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn the_chats_tracked_stay_within_the_cap() {
        let chats = ChatRateLimits::default().max_tracked_chats(3).idle_ttl(Duration::from_secs(60));
        let limiter = chat_limiter(chats);
        for chat_id in 1..=10 {
            limiter.acquire_chat(RateCategory::Message, chat_id).await;
            assert!(limiter.tracked_chats() <= 3, "{chat_id}: {}", limiter.tracked_chats());
        }
        assert_eq!(limiter.tracked_chats(), 3);
        // Once idle long enough, they all go when the next chat comes.
        tokio::time::advance(Duration::from_secs(61)).await;
        limiter.acquire_chat(RateCategory::Message, 11).await;
        assert_eq!(limiter.tracked_chats(), 1);

        let invalid = [
            ChatRateLimits::default().max_tracked_chats(0),
            ChatRateLimits::default().group_per_minute(Some(0)),
            ChatRateLimits::default().private(0.0, 1.0),
            ChatRateLimits::default().group(1.0, 0.5),
        ];
        for chats in invalid {
            assert!(RateLimits::default().per_chat(chats.clone()).validate().is_err(), "{chats:?}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_chat_in_use_is_never_evicted() {
        let limiter = Arc::new(chat_limiter(ChatRateLimits::default().private(0.001, 1.0).max_tracked_chats(1)));
        limiter.acquire_chat(RateCategory::Message, 7).await;
        let waiting = tokio::spawn({
            let limiter = Arc::clone(&limiter);
            async move { limiter.acquire_chat(RateCategory::Message, 7).await }
        });
        tokio::task::yield_now().await;
        limiter.acquire_chat(RateCategory::Message, 8).await;
        // Chat 7's bucket is in use, so both stay, past the cap, rather than chat 7 starting again from full.
        assert_eq!(limiter.tracked_chats(), 2);
        assert!(!limiter.try_acquire_chat(RateCategory::Message, 7));
        waiting.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_penalty_holds_back_only_its_chat() {
        let limiter = chat_limiter(ChatRateLimits::default());
        let start = Instant::now();
        limiter.penalize_chat(7, Duration::from_secs(30));
        limiter.acquire_chat(RateCategory::Message, 8).await;
        limiter.acquire_chat(RateCategory::Message, -5).await;
        limiter.acquire(RateCategory::Message).await;
        assert_eq!(start.elapsed(), Duration::ZERO, "another chat waited");
        assert!(!limiter.try_acquire_chat(RateCategory::Message, 7));
        limiter.acquire_chat(RateCategory::Message, 7).await;
        // The call that met the wait goes again by itself when it ends, and the next one a token's time later.
        assert!(near(start.elapsed(), Duration::from_secs(31)), "{:?}", start.elapsed());
        assert_eq!(limiter.tracked_chats(), 3);
    }
}
