use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::messages::caption_setters;
use crate::raw::{base, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct PlaceGiftAuctionBid<'a> {
    client: &'a Client,
    gift_id: i64,
    star_count: i64,
    recipient: Option<PeerRef>,
    text: Option<String>,
    parse_mode: Option<ParseMode>,
    private: bool,
}

impl Client {
    /// Bids `star_count` Stars in a gift's auction for the user or, with `.recipient()`, for someone else; nothing
    /// is paid above `star_count`.
    pub fn place_gift_auction_bid(&self, gift_id: i64, star_count: i64) -> PlaceGiftAuctionBid<'_> {
        PlaceGiftAuctionBid {
            client: self,
            gift_id,
            star_count,
            recipient: None,
            text: None,
            parse_mode: None,
            private: false,
        }
    }
}

impl PlaceGiftAuctionBid<'_> {
    pub fn recipient(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.recipient = Some(chat_id.into());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }

    async fn send(self) -> Result<()> {
        if self.star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        let client = self.client;
        let peer = match self.recipient {
            Some(chat) => client.resolve_peer(chat).await?,
            None => base::InputPeer::InputPeerSelf,
        };
        let invoice = types::InputInvoiceStarGiftAuctionBid {
            hide_name: self.private,
            update_bid: false,
            peer: Some(peer),
            gift_id: self.gift_id,
            bid_amount: self.star_count,
            message: client.parse_text_with_entities(self.text.as_deref(), self.parse_mode).await?,
        };
        let invoice = base::InputInvoice::InputInvoiceStarGiftAuctionBid(Box::new(invoice));
        client.pay_in_stars_up_to(invoice, self.star_count).await
    }
}

caption_setters!(PlaceGiftAuctionBid);
into_future!(PlaceGiftAuctionBid, ());

#[cfg(test)]
pub(super) mod tests {
    use super::super::send_payment_form::tests::{client, gift_form, paid, payment};
    use crate::errors::Error;
    use crate::raw::{base, types};

    pub(in crate::methods::payments) fn bid(
        update_bid: bool,
        hide_name: bool,
        peer: Option<base::InputPeer>,
        message: Option<&str>,
    ) -> base::InputInvoice {
        let message = message.map(|text| {
            let text = types::TextWithEntities { text: text.into(), entities: Vec::new() };
            base::TextWithEntities::TextWithEntities(Box::new(text))
        });
        let invoice =
            types::InputInvoiceStarGiftAuctionBid { hide_name, update_bid, peer, gift_id: 1, bid_amount: 900, message };
        base::InputInvoice::InputInvoiceStarGiftAuctionBid(Box::new(invoice))
    }

    #[tokio::test]
    async fn bids_up_to_the_cap() {
        let me = Some(base::InputPeer::InputPeerSelf);
        let user = Some(base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 })));
        let replies = (0..4).flat_map(|_| [gift_form("XTR", 900), paid()]).collect();
        let (client, mut sent) = client(replies);
        client.place_gift_auction_bid(1, 900).await.unwrap();
        client.place_gift_auction_bid(1, 900).recipient(5).await.unwrap();
        client.place_gift_auction_bid(1, 900).private().await.unwrap();
        client.place_gift_auction_bid(1, 900).text("hi").await.unwrap();
        let expected = [
            bid(false, false, me.clone(), None),
            bid(false, false, user, None),
            bid(false, true, me.clone(), None),
            bid(false, false, me, Some("hi")),
        ];
        for invoice in expected {
            let [get, pay] = payment(invoice);
            assert_eq!(sent.try_recv().unwrap(), get);
            assert_eq!(sent.try_recv().unwrap(), pay);
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_price_above_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 901)]);
        assert!(matches!(client.place_gift_auction_bid(1, 900).await, Err(Error::InvalidArgument(_))));
        let invoice = bid(false, false, Some(base::InputPeer::InputPeerSelf), None);
        assert_eq!(sent.try_recv().unwrap(), payment(invoice)[0]);
        assert!(sent.try_recv().is_err(), "nothing may be paid above star_count");
    }

    #[tokio::test]
    async fn a_non_positive_cap_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        // 777 is not a known peer, so resolving it would send a request.
        for stars in [0, -1] {
            let refused = client.place_gift_auction_bid(1, stars).recipient(777).await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{stars}");
        }
        assert!(sent.try_recv().is_err());
    }
}
