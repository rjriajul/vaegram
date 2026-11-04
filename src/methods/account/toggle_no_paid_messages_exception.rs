use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

/// Lets a user message you for free (or, with [`require_payment`](Self::require_payment), takes that back).
#[must_use = "a request does nothing until it is awaited"]
pub struct ToggleNoPaidMessagesException<'a> {
    client: &'a Client,
    user_id: PeerRef,
    require_payment: bool,
    refund_charged: bool,
    chat_id: Option<PeerRef>,
}

impl Client {
    pub fn toggle_no_paid_messages_exception(&self, user_id: impl Into<PeerRef>) -> ToggleNoPaidMessagesException<'_> {
        ToggleNoPaidMessagesException {
            client: self,
            user_id: user_id.into(),
            require_payment: false,
            refund_charged: false,
            chat_id: None,
        }
    }
}

impl ToggleNoPaidMessagesException<'_> {
    /// Removes the exception: the user pays for messages again.
    pub fn require_payment(mut self) -> Self {
        self.require_payment = true;
        self
    }

    /// Gives the user back the Stars they already paid.
    pub fn refund_charged(mut self) -> Self {
        self.refund_charged = true;
        self
    }

    /// The channel whose direct messages the exception is for.
    pub fn chat_id(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.chat_id = Some(chat_id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let parent_peer = match &self.chat_id {
            Some(chat_id) => Some(self.client.resolve_peer(chat_id.clone()).await?),
            None => None,
        };
        let request = functions::account::ToggleNoPaidMessagesException {
            refund_charged: self.refund_charged,
            require_payment: self.require_payment,
            parent_peer,
            user_id: self.client.resolve_user(&self.user_id).await?,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(ToggleNoPaidMessagesException, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn exception_sets_flags_and_parent_peer() {
        let (client, mut sent) = Client::recording();
        {
            let mut peers = client.0.peers.lock().unwrap();
            peers.insert(
                9,
                base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 9, access_hash: 1 })),
            );
            let channel = crate::utils::get_channel_id(5);
            let peer = types::InputPeerChannel { channel_id: 5, access_hash: 2 };
            peers.insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        }
        let channel = crate::utils::get_channel_id(5);
        let user = || base::InputUser::InputUser(Box::new(types::InputUser { user_id: 9, access_hash: 1 }));
        let _ = client.toggle_no_paid_messages_exception(9_i64).await;
        let plain = functions::account::ToggleNoPaidMessagesException {
            refund_charged: false,
            require_payment: false,
            parent_peer: None,
            user_id: user(),
        };
        assert_eq!(sent.try_recv().unwrap(), plain.to_bytes());

        let _ =
            client.toggle_no_paid_messages_exception(9_i64).require_payment().refund_charged().chat_id(channel).await;
        let peer = types::InputPeerChannel { channel_id: 5, access_hash: 2 };
        let full = functions::account::ToggleNoPaidMessagesException {
            refund_charged: true,
            require_payment: true,
            parent_peer: Some(base::InputPeer::InputPeerChannel(Box::new(peer))),
            user_id: user(),
        };
        assert_eq!(sent.try_recv().unwrap(), full.to_bytes());
    }
}
