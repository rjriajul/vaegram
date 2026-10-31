use super::send_resold_gift::gift_slug;
use super::set_gift_resale_price::stars_amount;
use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{PeerRef, StarAmount};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendGiftPurchaseOffer<'a> {
    client: &'a Client,
    owner_id: PeerRef,
    gift: &'a str,
    price: StarAmount,
    duration: i32,
    paid_message_star_count: Option<i64>,
}

impl Client {
    /// Offers `price` for an upgraded gift owned by `owner_id`, open for `duration` seconds; the Stars are charged if
    /// the owner accepts. `gift` is the gift's name or its `t.me/nft/<name>` link.
    pub fn send_gift_purchase_offer<'a>(
        &'a self,
        owner_id: impl Into<PeerRef>,
        gift: &'a str,
        price: StarAmount,
        duration: i32,
    ) -> SendGiftPurchaseOffer<'a> {
        SendGiftPurchaseOffer {
            client: self,
            owner_id: owner_id.into(),
            gift,
            price,
            duration,
            paid_message_star_count: None,
        }
    }
}

impl SendGiftPurchaseOffer<'_> {
    /// The most Stars to pay if the owner charges for messages.
    pub fn paid_message_star_count(mut self, star_count: i64) -> Self {
        self.paid_message_star_count = Some(star_count);
        self
    }

    async fn send(self) -> Result<()> {
        let slug = gift_slug(self.gift)?.to_owned();
        let price = stars_amount(self.price)?;
        if self.paid_message_star_count.is_some_and(|stars| stars <= 0) {
            return Err(Error::InvalidArgument("paid_message_star_count must be positive"));
        }
        let request = functions::payments::SendStarGiftOffer {
            peer: self.client.resolve_peer(self.owner_id).await?,
            slug,
            price,
            duration: self.duration,
            random_id: i64::from_le_bytes(random()),
            allow_paid_stars: self.paid_message_star_count,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SendGiftPurchaseOffer, ());

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::StarAmount;

    fn stars(amount: i64) -> StarAmount {
        StarAmount { amount, nanostar_amount: 0, ton: false }
    }

    fn offer(allow_paid_stars: Option<i64>) -> Vec<u8> {
        let request = functions::payments::SendStarGiftOffer {
            peer: base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 })),
            slug: "PlushPepe-3".into(),
            price: base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 500, nanos: 0 })),
            duration: 86400,
            random_id: 0,
            allow_paid_stars,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn sends_the_offer() {
        let replies = (0..2).map(|_| base::Updates::UpdatesTooLong.to_bytes()).collect();
        let (client, mut sent) = client(replies);
        client.send_gift_purchase_offer(5, "t.me/nft/PlushPepe-3", stars(500), 86400).await.unwrap();
        // random_id is random, so only the bytes around it are compared.
        let (body, request) = (sent.try_recv().unwrap(), offer(None));
        let fixed = request.len() - 8;
        assert_eq!((body.len(), &body[..fixed]), (request.len(), &request[..fixed]));

        client.send_gift_purchase_offer(5, "PlushPepe-3", stars(500), 86400).paid_message_star_count(10).await.unwrap();
        let (body, request) = (sent.try_recv().unwrap(), offer(Some(10)));
        let (head, tail) = (request.len() - 16, request.len() - 8);
        assert_eq!((body.len(), &body[..head], &body[tail..]), (request.len(), &request[..head], &request[tail..]));
    }

    #[tokio::test]
    async fn misuse_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        // 777 is not a known peer, so resolving it would send a request.
        let refused = [
            client.send_gift_purchase_offer(777, "t.me/nft/", stars(500), 86400).await,
            client.send_gift_purchase_offer(777, "PlushPepe-3", stars(0), 86400).await,
            client.send_gift_purchase_offer(777, "PlushPepe-3", stars(-1), 86400).await,
            client.send_gift_purchase_offer(777, "PlushPepe-3", stars(500), 86400).paid_message_star_count(0).await,
        ];
        for (i, result) in refused.into_iter().enumerate() {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{i}");
        }
        assert!(sent.try_recv().is_err());
    }
}
