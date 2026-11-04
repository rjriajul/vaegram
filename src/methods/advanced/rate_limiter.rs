use crate::Client;
use crate::session::RateLimiter;

impl Client {
    /// The client's rate limiter, if `ClientBuilder::rate_limits` turned it on: to see how busy it is
    /// (`congestion`), whether a call would wait (`available`, `try_acquire`), or to change a limit while the client
    /// runs (`update_limits`).
    pub fn rate_limiter(&self) -> Option<&RateLimiter> {
        self.0.limiter.as_ref()
    }
}
