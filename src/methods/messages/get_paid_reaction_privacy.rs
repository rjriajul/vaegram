use crate::Client;
use crate::enums::PaidReactionPrivacy;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};

impl Client {
    /// Returns how the account's paid reactions are shown by default.
    ///
    /// The setting applies to [`send_paid_reaction`](Client::send_paid_reaction) calls that don't pick their own
    /// privacy, and changes with [`toggle_paid_reaction_privacy`](Client::toggle_paid_reaction_privacy).
    pub async fn get_paid_reaction_privacy(&self) -> Result<PaidReactionPrivacy> {
        let updates = match self.invoke(&functions::messages::GetPaidReactionPrivacy {}).await? {
            base::Updates::Updates(u) => u.updates,
            base::Updates::UpdatesCombined(u) => u.updates,
            base::Updates::UpdateShort(u) => vec![u.update],
            _ => Vec::new(),
        };
        let me = self.0.me.lock().unwrap().id;
        updates
            .iter()
            .find_map(|update| match update {
                base::Update::UpdatePaidReactionPrivacy(u) => Some(PaidReactionPrivacy::parse(&u.private, me)),
                _ => None,
            })
            .ok_or(Error::Unexpected("Telegram sent no paid reaction privacy"))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::PaidReactionPrivacy;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::utils::get_channel_id;

    fn reply(private: base::PaidReactionPrivacy) -> Vec<u8> {
        let update = types::UpdatePaidReactionPrivacy { private };
        let updates = types::Updates {
            updates: vec![base::Update::UpdatePaidReactionPrivacy(Box::new(update))],
            users: Vec::new(),
            chats: Vec::new(),
            date: 1,
            seq: 0,
        };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    #[tokio::test]
    async fn reads_each_kind_of_privacy_from_the_update() {
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let peer = types::PaidReactionPrivacyPeer { peer: base::InputPeer::InputPeerChannel(Box::new(channel)) };
        let replies = vec![
            Ok(reply(base::PaidReactionPrivacy::PaidReactionPrivacyDefault)),
            Ok(reply(base::PaidReactionPrivacy::PaidReactionPrivacyAnonymous)),
            Ok(reply(base::PaidReactionPrivacy::PaidReactionPrivacyPeer(Box::new(peer)))),
        ];
        let (client, mut sent) = Client::scripted(replies);
        assert_eq!(client.get_paid_reaction_privacy().await.unwrap(), PaidReactionPrivacy::Default);
        assert_eq!(client.get_paid_reaction_privacy().await.unwrap(), PaidReactionPrivacy::Anonymous);
        let chat = PaidReactionPrivacy::Chat(get_channel_id(9).into());
        assert_eq!(client.get_paid_reaction_privacy().await.unwrap(), chat);
        for _ in 0..3 {
            assert_eq!(sent.try_recv().unwrap(), functions::messages::GetPaidReactionPrivacy {}.to_bytes());
        }
    }

    #[tokio::test]
    async fn an_answer_without_the_update_is_unexpected() {
        let empty = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let (client, _sent) = Client::scripted(vec![Ok(base::Updates::Updates(Box::new(empty)).to_bytes())]);
        let result = client.get_paid_reaction_privacy().await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
    }
}
