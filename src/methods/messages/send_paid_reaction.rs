use crate::Client;
use crate::crypto::random;
use crate::enums::PaidReactionPrivacy;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::session::internals::now;
use crate::types::PeerRef;

/// Request built by [`Client::send_paid_reaction`]. Await it to send.
#[must_use = "a request does nothing until it is awaited"]
pub struct SendPaidReaction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    star_count: i32,
    privacy: Option<PaidReactionPrivacy>,
}

impl Client {
    /// Sends a paid reaction worth `star_count` Stars to a message.
    ///
    /// A `star_count` below 1 fails with `Error::InvalidArgument`. Awaiting the request returns `()`.
    pub fn send_paid_reaction(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        star_count: i32,
    ) -> SendPaidReaction<'_> {
        SendPaidReaction { client: self, chat_id: chat_id.into(), message_id, star_count, privacy: None }
    }
}

impl SendPaidReaction<'_> {
    /// Chooses who the reaction is shown as. Without it the account's own setting applies.
    pub fn privacy(mut self, privacy: PaidReactionPrivacy) -> Self {
        self.privacy = Some(privacy);
        self
    }

    async fn send(self) -> Result<()> {
        if self.star_count < 1 {
            return Err(Error::InvalidArgument("a paid reaction takes at least 1 star"));
        }
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let private = match self.privacy {
            Some(privacy) => Some(privacy.write(self.client).await?),
            None => None,
        };
        let request = functions::messages::SendPaidReaction {
            peer,
            msg_id: self.message_id,
            count: self.star_count,
            random_id: random_id(now(), random()),
            private,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SendPaidReaction, ());

// Telegram wants the unix time in the high 32 bits and random bits in the low ones.
fn random_id(unix_time: i64, low: [u8; 4]) -> i64 {
    (unix_time << 32) | i64::from(u32::from_le_bytes(low))
}

#[cfg(test)]
mod tests {
    use super::random_id;
    use crate::enums::PaidReactionPrivacy;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, types};

    async fn recorded(privacy: Option<PaidReactionPrivacy>) -> Vec<u8> {
        let (client, mut rx) = crate::Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let mut request = client.send_paid_reaction(7, 1, 5);
        if let Some(privacy) = privacy {
            request = request.privacy(privacy);
        }
        let _ = request.await;
        rx.try_recv().expect("no request sent")
    }

    fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }))
    }

    #[tokio::test]
    async fn paid_reaction_privacy_writes_each_kind() {
        use base::PaidReactionPrivacy as P;
        let kinds = [
            (PaidReactionPrivacy::Default, P::PaidReactionPrivacyDefault),
            (PaidReactionPrivacy::Anonymous, P::PaidReactionPrivacyAnonymous),
            (
                PaidReactionPrivacy::Chat(7.into()),
                P::PaidReactionPrivacyPeer(Box::new(types::PaidReactionPrivacyPeer { peer: peer() })),
            ),
        ];
        for (privacy, raw) in kinds {
            let bytes = recorded(Some(privacy)).await;
            let tail = raw.to_bytes();
            assert!(bytes.ends_with(&tail), "{bytes:?}");
            assert_eq!(bytes[4] & 1, 1, "the private flag is set");
        }
    }

    #[tokio::test]
    async fn no_privacy_sends_the_account_setting() {
        let bytes = recorded(None).await;
        assert_eq!(bytes[4] & 1, 0, "the private flag is clear");
    }

    #[test]
    fn random_id_puts_the_time_above_the_random_bits() {
        assert_eq!(random_id(1_700_000_000, [0xff; 4]), (1_700_000_000 << 32) | 0xffff_ffff);
        assert_eq!(random_id(1_700_000_000, [1, 0, 0, 0]) >> 32, 1_700_000_000);
    }

    #[tokio::test]
    async fn no_stars_is_refused_without_a_request() {
        let (client, mut rx) = crate::Client::scripted(Vec::new());
        for stars in [0, -1] {
            assert!(matches!(client.send_paid_reaction(7, 1, stars).await, Err(Error::InvalidArgument(_))));
        }
        assert!(rx.try_recv().is_err());
    }
}
