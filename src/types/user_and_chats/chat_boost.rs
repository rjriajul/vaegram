use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, User};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatBoost {
    pub boost_id: String,
    pub add_date: i32,
    pub expiration_date: i32,
    pub source: ChatBoostSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChatBoostSource {
    Premium {
        user: User,
    },
    GiftCode {
        user: User,
    },
    /// A giveaway created by the chat; `user` is the winner, unless the prize went unclaimed.
    Giveaway {
        giveaway_message_id: i32,
        user: Option<User>,
        prize_star_count: Option<i64>,
        is_unclaimed: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ChatBoostUpdated {
    pub chat: Chat,
    pub boost: ChatBoost,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct ChatBoostRemoved {
    pub chat: Chat,
    pub boost_id: String,
    pub remove_date: i32,
    pub source: ChatBoostSource,
    client: Client,
}

impl ChatBoost {
    // Premium and gift-code boosts always name a booster; one without is malformed, so it's skipped.
    pub(crate) fn parse(raw: base::Boost, entities: &Entities) -> Option<ChatBoost> {
        let base::Boost::Boost(raw) = raw;
        let user = raw.user_id.map(|id| User::parse(id, entities));
        let source = if raw.giveaway {
            ChatBoostSource::Giveaway {
                giveaway_message_id: raw.giveaway_msg_id.unwrap_or_default(),
                user: user.filter(|_| !raw.unclaimed),
                prize_star_count: raw.stars.filter(|&stars| stars > 0),
                is_unclaimed: raw.unclaimed,
            }
        } else if raw.gift {
            ChatBoostSource::GiftCode { user: user? }
        } else {
            ChatBoostSource::Premium { user: user? }
        };
        Some(ChatBoost { boost_id: raw.id, add_date: raw.date, expiration_date: raw.expires, source })
    }
}

impl ChatBoostSource {
    pub fn user(&self) -> Option<&User> {
        match self {
            ChatBoostSource::Premium { user } | ChatBoostSource::GiftCode { user } => Some(user),
            ChatBoostSource::Giveaway { user, .. } => user.as_ref(),
        }
    }
}

impl ChatBoostUpdated {
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl ChatBoostRemoved {
    pub fn client(&self) -> &Client {
        &self.client
    }
}

/// A changed boost or a removed one: Telegram sends both as `updateBotChatBoost`.
pub(crate) enum BoostUpdate {
    Updated(ChatBoostUpdated),
    Removed(ChatBoostRemoved),
}

impl BoostUpdate {
    pub(crate) fn parse(client: &Client, raw: types::UpdateBotChatBoost, entities: &Entities) -> Option<BoostUpdate> {
        let (chat, client) = (Chat::parse(&raw.peer, entities, false), client.clone());
        let boost = ChatBoost::parse(raw.boost, entities)?;
        // A removed boost comes with expires 0, dated when it was removed.
        Some(match boost.expiration_date {
            0 => BoostUpdate::Removed(ChatBoostRemoved {
                chat,
                boost_id: boost.boost_id,
                remove_date: boost.add_date,
                source: boost.source,
                client,
            }),
            _ => BoostUpdate::Updated(ChatBoostUpdated { chat, boost, client }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BoostUpdate, ChatBoost, ChatBoostSource};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::User;
    use crate::utils::get_peer;

    fn boost(gift: bool, giveaway: bool, unclaimed: bool, user_id: Option<i64>) -> base::Boost {
        base::Boost::Boost(Box::new(types::Boost {
            gift,
            giveaway,
            unclaimed,
            id: "b".into(),
            user_id,
            giveaway_msg_id: giveaway.then_some(9),
            date: 1,
            expires: 2,
            used_gift_slug: None,
            multiplier: None,
            stars: None,
        }))
    }

    #[test]
    fn maps_boost_sources() {
        let entities = Entities::default();
        let parse = |raw| ChatBoost::parse(raw, &entities).map(|b| b.source);
        let user = User { id: 5, ..User::default() };

        assert_eq!(parse(boost(false, false, false, Some(5))), Some(ChatBoostSource::Premium { user: user.clone() }));
        assert_eq!(parse(boost(true, false, false, Some(5))), Some(ChatBoostSource::GiftCode { user: user.clone() }));
        assert_eq!(parse(boost(false, false, false, None)), None);
        let giveaway = |user, is_unclaimed| ChatBoostSource::Giveaway {
            giveaway_message_id: 9,
            user,
            prize_star_count: None,
            is_unclaimed,
        };
        assert_eq!(parse(boost(false, true, false, Some(5))), Some(giveaway(Some(user), false)));
        assert_eq!(parse(boost(false, true, true, Some(5))), Some(giveaway(None, true)));
    }

    #[test]
    fn a_boost_without_expiry_is_removed() {
        let client = Client::offline();
        let update = |expires| {
            let base::Boost::Boost(mut raw) = boost(false, false, false, Some(5));
            raw.expires = expires;
            let (peer, boost) = (get_peer(-1000000000005), base::Boost::Boost(raw));
            BoostUpdate::parse(&client, types::UpdateBotChatBoost { peer, boost, qts: 1 }, &Entities::default())
        };
        assert!(matches!(update(2), Some(BoostUpdate::Updated(u)) if u.boost.expiration_date == 2));
        assert!(matches!(
            update(0),
            Some(BoostUpdate::Removed(r)) if r.remove_date == 1 && r.chat.id == -1000000000005
        ));
    }
}
