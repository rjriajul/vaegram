use crate::Client;
use crate::session::RateLimiter;

impl Client {
    /// The client's rate limiter: the one `ClientBuilder::rate_limits` set, else a user account's default one
    /// ([`RateLimits::user_account`](crate::session::RateLimits::user_account)); `None` for a bot given none. To
    /// see how busy it is (`congestion`), whether a call would wait (`available`, `try_acquire`,
    /// `try_acquire_chat`), how many chats it keeps limits for (`tracked_chats`), or to change a limit while the
    /// client runs (`update_limits`).
    pub fn rate_limiter(&self) -> Option<&RateLimiter> {
        self.0.limiter.as_ref()
    }
}
