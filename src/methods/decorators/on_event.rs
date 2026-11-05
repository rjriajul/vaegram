use std::sync::Arc;

use crate::Client;
use crate::enums::ClientEvent;

impl Client {
    /// It runs on the connection's task, so keep it short or forward the events to a channel. A panic in it is
    /// caught; the connection carries on. `ClientEvent::StorageFailed` may also arrive from the storage engine's
    /// own thread, or from the task that asked for a peer.
    ///
    /// With no listener, an unhandled handler error and a failed storage call are printed to stderr as a line
    /// starting with `vaegram:`; with one, they only come here. A listener can only be set once the client is
    /// built, so what the storage reports while it opens is always printed.
    pub fn on_event<F>(&self, callback: F)
    where
        F: Fn(ClientEvent) + Send + Sync + 'static,
    {
        self.0.events.listen(Arc::new(callback));
    }
}
