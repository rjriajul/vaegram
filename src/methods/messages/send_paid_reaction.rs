use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::session::internals::now;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SendPaidReaction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    star_count: i32,
    anonymous: bool,
}

impl Client {
    pub fn send_paid_reaction(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        star_count: i32,
    ) -> SendPaidReaction<'_> {
        SendPaidReaction { client: self, chat_id: chat_id.into(), message_id, star_count, anonymous: false }
    }
}

impl SendPaidReaction<'_> {
    pub fn anonymous(mut self) -> Self {
        self.anonymous = true;
        self
    }

    async fn send(self) -> Result<()> {
        if self.star_count < 1 {
            return Err(Error::InvalidArgument("a paid reaction takes at least 1 star"));
        }
        let request = functions::messages::SendPaidReaction {
            peer: self.client.resolve_peer(self.chat_id).await?,
            msg_id: self.message_id,
            count: self.star_count,
            random_id: random_id(now(), random()),
            private: self.anonymous.then_some(base::PaidReactionPrivacy::PaidReactionPrivacyAnonymous),
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
    use crate::errors::Error;

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
