use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};

impl Client {
    /// Removes the sender, recipient and message from an upgraded gift; nothing is paid above `star_count`.
    pub async fn drop_gift_original_details(&self, owned_gift_id: &str, star_count: i64) -> Result<()> {
        if star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        if owned_gift_id.is_empty() {
            return Err(Error::InvalidArgument("owned_gift_id is empty"));
        }
        let stargift = self.input_saved_gift(owned_gift_id).await?;
        let invoice = types::InputInvoiceStarGiftDropOriginalDetails { stargift };
        let invoice = base::InputInvoice::InputInvoiceStarGiftDropOriginalDetails(Box::new(invoice));
        self.pay_in_stars_up_to(invoice, star_count).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::{client, gift_form, paid, payment};
    use crate::errors::Error;
    use crate::raw::{base, types};

    fn invoice() -> base::InputInvoice {
        let stargift = base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id: 9 });
        let invoice = types::InputInvoiceStarGiftDropOriginalDetails { stargift };
        base::InputInvoice::InputInvoiceStarGiftDropOriginalDetails(Box::new(invoice))
    }

    #[tokio::test]
    async fn pays_up_to_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 900), paid()]);
        client.drop_gift_original_details("9", 900).await.unwrap();
        let [get, pay] = payment(invoice());
        assert_eq!(sent.try_recv().unwrap(), get);
        assert_eq!(sent.try_recv().unwrap(), pay);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_price_above_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 901)]);
        assert!(matches!(client.drop_gift_original_details("9", 900).await, Err(Error::InvalidArgument(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(invoice())[0]);
        assert!(sent.try_recv().is_err(), "nothing may be paid above star_count");
    }

    #[tokio::test]
    async fn misuse_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        // "-100123_4" names a channel gift, which would need a lookup before the cap check.
        for (id, stars) in [("9", 0), ("-100123_4", -1), ("", 900)] {
            let refused = client.drop_gift_original_details(id, stars).await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{id:?} {stars}");
        }
        assert!(sent.try_recv().is_err());
    }
}
