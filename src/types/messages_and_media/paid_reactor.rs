use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::Chat;

/// One of the top senders of paid (Star) reactions on a message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PaidReactor {
    /// Who sent the Stars; `None` when they reacted anonymously.
    pub sender: Option<Chat>,
    /// Stars this sender has spent on the message.
    pub star_count: i32,
    /// The sender is among the top senders.
    pub is_top: bool,
    /// The current account sent them.
    pub is_me: bool,
    /// The sender chose to react anonymously.
    pub is_anonymous: bool,
}

impl PaidReactor {
    pub(crate) fn parse(raw: &base::MessageReactor, entities: &Entities) -> PaidReactor {
        let base::MessageReactor::MessageReactor(r) = raw;
        PaidReactor {
            sender: r.peer_id.as_ref().map(|peer| Chat::parse(peer, entities, false)),
            star_count: r.count,
            is_top: r.top,
            is_me: r.my,
            is_anonymous: r.anonymous,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PaidReactor;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::utils::get_peer;

    #[test]
    fn reads_named_and_anonymous_reactors() {
        let reactor = |peer_id, anonymous| {
            let raw = types::MessageReactor { top: true, my: false, anonymous, peer_id, count: 50 };
            PaidReactor::parse(&base::MessageReactor::MessageReactor(Box::new(raw)), &Entities::default())
        };
        let named = reactor(Some(get_peer(7)), false);
        assert_eq!((named.sender.map(|c| c.id), named.star_count, named.is_top), (Some(7), 50, true));
        let anonymous = reactor(None, true);
        assert_eq!((anonymous.sender, anonymous.is_anonymous, anonymous.is_me), (None, true, false));
    }
}
