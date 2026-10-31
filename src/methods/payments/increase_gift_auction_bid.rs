use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};

impl Client {
    /// Raises the user's bid in a gift's auction to `star_count` Stars; nothing is paid above `star_count`.
    pub async fn increase_gift_auction_bid(&self, gift_id: i64, star_count: i64) -> Result<()> {
        if star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        let invoice = types::InputInvoiceStarGiftAuctionBid {
            hide_name: false,
            update_bid: true,
            peer: None,
            gift_id,
            bid_amount: star_count,
            message: None,
        };
        let invoice = base::InputInvoice::InputInvoiceStarGiftAuctionBid(Box::new(invoice));
        self.pay_in_stars_up_to(invoice, star_count).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::place_gift_auction_bid::tests::bid;
    use super::super::send_payment_form::tests::{client, gift_form, paid, payment};
    use crate::errors::Error;

    #[tokio::test]
    async fn raises_up_to_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 900), paid()]);
        client.increase_gift_auction_bid(1, 900).await.unwrap();
        let [get, pay] = payment(bid(true, false, None, None));
        assert_eq!(sent.try_recv().unwrap(), get);
        assert_eq!(sent.try_recv().unwrap(), pay);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_price_above_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 901)]);
        assert!(matches!(client.increase_gift_auction_bid(1, 900).await, Err(Error::InvalidArgument(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(bid(true, false, None, None))[0]);
        assert!(sent.try_recv().is_err(), "nothing may be paid above star_count");
    }

    #[tokio::test]
    async fn a_non_positive_cap_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        for stars in [0, -1] {
            assert!(matches!(client.increase_gift_auction_bid(1, stars).await, Err(Error::InvalidArgument(_))));
        }
        assert!(sent.try_recv().is_err());
    }
}
