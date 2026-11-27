use crate::Client;

impl Client {
    /// Provides `value` to the middleware and handlers of every included [`Router`](crate::handlers::Router), which
    /// take it as a [`Dep<V>`](crate::handlers::Dep). A router's or a middleware's value of the same type wins over
    /// it. Providing another value of the same type replaces it, from the next update on.
    pub fn provide<V: Send + Sync + 'static>(&self, value: V) {
        self.0.handlers.add(|r| r.layer.data.insert(value));
    }
}
