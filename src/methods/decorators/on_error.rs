use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Client;
use crate::handlers::HandlerError;

impl Client {
    /// Receives every `Err` a handler returns. An error also stops the later groups for that
    /// update. Without this, the error goes to [`on_event`](Client::on_event) as `ClientEvent::HandlerFailed`, and
    /// when that isn't set either, it is printed to stderr. Registering again replaces it. A panicking handler stops
    /// the later groups too, but its panic goes to `on_event` as `ClientEvent::HandlerPanicked`, never here.
    pub fn on_error<F, Fut>(&self, f: F)
    where
        F: Fn(HandlerError) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let f = Arc::new(move |e: HandlerError| Box::pin(f(e)) as Pin<Box<dyn Future<Output = ()> + Send>>);
        self.0.handlers.add(|r| r.on_error = Some(f));
    }
}
