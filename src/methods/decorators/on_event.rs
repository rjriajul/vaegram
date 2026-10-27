use std::sync::Arc;

use crate::Client;
use crate::enums::ClientEvent;

impl Client {
    /// It runs on the connection's task, so keep it short or forward the events to a channel. A panic in it is
    /// caught; the connection carries on.
    pub fn on_event<F>(&self, callback: F)
    where
        F: Fn(ClientEvent) + Send + Sync + 'static,
    {
        self.0.events.listen(Arc::new(callback));
    }
}
