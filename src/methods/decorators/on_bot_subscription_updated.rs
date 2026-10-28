use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::BotSubscriptionUpdated;

impl Client {
    pub fn on_bot_subscription_updated<F, Fut, T>(&self, filter: Filter<BotSubscriptionUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(BotSubscriptionUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::BotSubscriptionUpdated, |r, id| {
            r.bot_subscription_updated.push(id, filter, callback(f))
        })
    }
}
