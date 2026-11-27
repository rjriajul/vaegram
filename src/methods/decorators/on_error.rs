use std::future::Future;

use crate::Client;
use crate::handlers::{HandlerError, error_callback};

impl Client {
    /// Receives every `Err` a handler returns. An error also stops the later groups for that
    /// update. Without this, the error goes to [`on_event`](Client::on_event) as `ClientEvent::HandlerFailed`, and
    /// when that isn't set either, it is printed to stderr. Registering again replaces it. A panicking handler stops
    /// the later groups too, but its panic goes to `on_event` as `ClientEvent::HandlerPanicked`, never here. An
    /// included [`Router`](crate::handlers::Router) with an `on_error` of its own keeps its handlers' errors.
    pub fn on_error<F, Fut>(&self, f: F)
    where
        F: Fn(HandlerError) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let f = error_callback(f);
        self.0.handlers.add(|r| r.on_error = Some(f));
    }
}
