use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::User;

/// A Toncoin transfer sent through the Telegram wallet.
#[derive(Debug, Clone, PartialEq)]
pub struct TonWalletTransfer {
    /// The sender, or the chat's user when the message doesn't say.
    pub sender: Option<User>,
    pub transaction_id: String,
    pub peer_address: String,
    /// In nanotons.
    pub amount: i64,
    pub comment: Option<String>,
    pub is_comment_encrypted: bool,
}

impl TonWalletTransfer {
    pub(crate) fn parse(
        raw: &types::MessageActionGramTransfer,
        from_id: Option<&base::Peer>,
        peer_id: &base::Peer,
        entities: &Entities,
    ) -> TonWalletTransfer {
        let sender = match from_id.unwrap_or(peer_id) {
            base::Peer::PeerUser(p) => Some(User::parse(p.user_id, entities)),
            _ => None,
        };
        TonWalletTransfer {
            sender,
            transaction_id: raw.transaction_id.clone(),
            peer_address: raw.peer_address.clone(),
            amount: raw.amount,
            comment: raw.comment.clone(),
            is_comment_encrypted: raw.comment_encrypted,
        }
    }

    /// The transaction on a block explorer.
    pub fn link(&self) -> String {
        format!("https://tonviewer.com/transaction/{}", self.transaction_id)
    }
}

#[cfg(test)]
mod tests {
    use super::TonWalletTransfer;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    fn raw() -> types::MessageActionGramTransfer {
        types::MessageActionGramTransfer {
            comment_encrypted: true,
            amount: 5,
            peer_address: "EQa".into(),
            transaction_id: "abc".into(),
            comment: Some("hi".into()),
        }
    }

    fn user(id: i64) -> base::Peer {
        base::Peer::PeerUser(types::PeerUser { user_id: id })
    }

    #[test]
    fn transfer_link_and_fields() {
        let t = TonWalletTransfer::parse(&raw(), Some(&user(7)), &user(8), &Entities::default());
        assert_eq!(t.link(), "https://tonviewer.com/transaction/abc");
        assert_eq!((t.amount, t.comment.as_deref(), t.is_comment_encrypted), (5, Some("hi"), true));
        assert_eq!(t.sender.map(|u| u.id), Some(7));
    }

    #[test]
    fn sender_falls_back_to_the_chat_user() {
        let t = TonWalletTransfer::parse(&raw(), None, &user(8), &Entities::default());
        assert_eq!(t.sender.map(|u| u.id), Some(8));
        let channel = base::Peer::PeerChannel(types::PeerChannel { channel_id: 3 });
        assert!(TonWalletTransfer::parse(&raw(), None, &channel, &Entities::default()).sender.is_none());
    }
}
