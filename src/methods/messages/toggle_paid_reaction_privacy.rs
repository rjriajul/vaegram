use crate::Client;
use crate::enums::PaidReactionPrivacy;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Changes how the paid reactions the account already sent to one message are shown.
    pub async fn toggle_paid_reaction_privacy(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        privacy: PaidReactionPrivacy,
    ) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        let private = privacy.write(self).await?;
        let request = functions::messages::TogglePaidReactionPrivacy { peer, msg_id: message_id, private };
        accepted(self.invoke(&request).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::PaidReactionPrivacy;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    fn chat() -> base::InputPeer {
        base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 })
    }

    #[tokio::test]
    async fn sends_the_privacy_for_the_message() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes())]);
        client.toggle_paid_reaction_privacy(-5_i64, 9, PaidReactionPrivacy::Anonymous).await.unwrap();
        let private = base::PaidReactionPrivacy::PaidReactionPrivacyAnonymous;
        let expected = functions::messages::TogglePaidReactionPrivacy { peer: chat(), msg_id: 9, private };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn a_false_answer_is_an_error() {
        let (client, _sent) = Client::scripted_raw(vec![Ok(false.to_bytes())]);
        let result = client.toggle_paid_reaction_privacy(-5_i64, 9, PaidReactionPrivacy::Default).await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
    }
}
