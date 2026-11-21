use crate::raw::base;
use crate::types::PeerRef;
use crate::utils::input_peer_id;

/// How a paid reaction is shown in the top reactors of a message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum PaidReactionPrivacy {
    /// Shown under the account's own name and photo.
    Default,
    /// Shown as an anonymous reactor.
    Anonymous,
    /// Shown as the given chat, such as a channel the account owns.
    Chat(PeerRef),
}

impl PaidReactionPrivacy {
    /// Reads the setting Telegram sends. `me` is the id of the current account, for a chat named as itself.
    pub(crate) fn parse(raw: &base::PaidReactionPrivacy, me: i64) -> PaidReactionPrivacy {
        use base::PaidReactionPrivacy as P;
        match raw {
            P::PaidReactionPrivacyDefault => PaidReactionPrivacy::Default,
            P::PaidReactionPrivacyAnonymous => PaidReactionPrivacy::Anonymous,
            P::PaidReactionPrivacyPeer(p) => match input_peer_id(&p.peer, me) {
                Some(id) => PaidReactionPrivacy::Chat(PeerRef::Id(id)),
                None => PaidReactionPrivacy::Default,
            },
        }
    }

    pub(crate) async fn write(self, client: &crate::Client) -> crate::Result<base::PaidReactionPrivacy> {
        use base::PaidReactionPrivacy as P;
        Ok(match self {
            PaidReactionPrivacy::Default => P::PaidReactionPrivacyDefault,
            PaidReactionPrivacy::Anonymous => P::PaidReactionPrivacyAnonymous,
            PaidReactionPrivacy::Chat(chat) => {
                P::PaidReactionPrivacyPeer(Box::new(crate::raw::types::PaidReactionPrivacyPeer {
                    peer: client.resolve_peer(chat).await?,
                }))
            }
        })
    }
}
