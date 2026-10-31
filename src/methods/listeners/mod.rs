use std::time::Duration;

use tokio::time::Instant;

use crate::Client;
use crate::dispatcher::listeners::Criteria;
use crate::errors::Result;
use crate::types::PeerRef;

macro_rules! listener_setters {
    ($builder:ident) => {
        listener_setters!($builder, pattern);

        impl<T: $crate::handlers::Listenable> $builder<'_, T> {
            /// Only updates this filter passes; replaces an earlier filter.
            pub fn filter(mut self, filter: $crate::filters::Filter<T>) -> Self {
                self.spec.criteria.filter = Some(::std::sync::Arc::new(filter));
                self
            }

            /// How long to wait; 5 minutes unless set.
            pub fn timeout(mut self, timeout: ::std::time::Duration) -> Self {
                self.spec.timeout = Some(timeout);
                self
            }

            /// Waits for as long as it takes.
            pub fn no_timeout(mut self) -> Self {
                self.spec.timeout = None;
                self
            }
        }

        impl $builder<'_, $crate::types::CallbackQuery> {
            /// The alert other users get when they press the button this listener is pinned to (with `user` and
            /// `message_id` or `inline_message_id`); their press then goes no further.
            pub fn unallowed_click_alert(mut self, text: impl Into<String>) -> Self {
                let text: String = text.into();
                self.spec.criteria.alert = Some(text.into());
                self
            }

            /// Leaves other users' presses on the pinned button to the handlers.
            pub fn no_unallowed_click_alert(mut self) -> Self {
                self.spec.criteria.alert = None;
                self
            }
        }
    };
    ($builder:ident, pattern) => {
        impl<T: $crate::handlers::Listenable> $builder<'_, T> {
            /// Only updates in this chat; call again to accept another chat too.
            pub fn chat(mut self, chat_id: impl Into<$crate::types::PeerRef>) -> Self {
                self.spec.chats.push(chat_id.into());
                self
            }

            /// Only updates from this user; call again to accept another user too. A chat that posts as itself
            /// counts as the user.
            pub fn user(mut self, user_id: impl Into<$crate::types::PeerRef>) -> Self {
                self.spec.users.push(user_id.into());
                self
            }
        }

        impl $builder<'_, $crate::types::CallbackQuery> {
            /// Only presses on this message's buttons; call again to accept another message too.
            pub fn message_id(mut self, message_id: i32) -> Self {
                self.spec.criteria.messages.push(message_id);
                self
            }

            /// Only presses on this inline message's buttons; call again to accept another one too.
            pub fn inline_message_id(mut self, id: $crate::types::InlineMessageId) -> Self {
                self.spec.criteria.inline.push(id);
                self
            }
        }
    };
}

mod ask;
mod listen;
mod register_next_step_handler;
mod stop_listening;
mod wait_for_callback_query;
mod wait_for_message;

pub use ask::Ask;
pub use listen::Listen;
pub use register_next_step_handler::RegisterNextStepHandler;
pub use stop_listening::StopListening;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// What a listener builder collects before its peers are resolved.
pub(crate) struct Spec<T> {
    chats: Vec<PeerRef>,
    users: Vec<PeerRef>,
    criteria: Criteria<T>,
    timeout: Option<Duration>,
}

impl<T> Default for Spec<T> {
    fn default() -> Self {
        Spec { chats: Vec::new(), users: Vec::new(), criteria: Criteria::default(), timeout: Some(DEFAULT_TIMEOUT) }
    }
}

impl<T> Spec<T> {
    async fn resolve(self, client: &Client) -> Result<Criteria<T>> {
        let mut criteria = self.criteria;
        for (refs, ids) in [(self.chats, &mut criteria.chats), (self.users, &mut criteria.users)] {
            for peer in refs {
                let id = client.peer_id(peer).await?;
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        // `Duration::MAX` overflows the clock: that means no deadline.
        criteria.deadline = self.timeout.and_then(|timeout| Instant::now().checked_add(timeout));
        Ok(criteria)
    }
}
