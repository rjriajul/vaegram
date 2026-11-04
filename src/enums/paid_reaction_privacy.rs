use crate::raw::base;
use crate::types::PeerRef;

/// How a paid reaction is shown in the top reactors of a message.
#[derive(Debug, Clone, PartialEq)]
pub enum PaidReactionPrivacy {
    /// Shown under the account's own name and photo.
    Default,
    /// Shown as an anonymous reactor.
    Anonymous,
    /// Shown as the given chat, such as a channel the account owns.
    Chat(PeerRef),
}

impl PaidReactionPrivacy {
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
