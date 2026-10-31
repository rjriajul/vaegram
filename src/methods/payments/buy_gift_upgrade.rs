use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::PeerRef;

impl Client {
    /// Pays for the upgrade of a gift someone else owns; nothing is paid above `star_count`.
    pub async fn buy_gift_upgrade(
        &self,
        owner_id: impl Into<PeerRef>,
        prepaid_upgrade_hash: &str,
        star_count: i64,
    ) -> Result<()> {
        if star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        if prepaid_upgrade_hash.is_empty() {
            return Err(Error::InvalidArgument("prepaid_upgrade_hash is empty"));
        }
        let invoice = types::InputInvoiceStarGiftPrepaidUpgrade {
            peer: self.resolve_peer(owner_id).await?,
            hash: prepaid_upgrade_hash.into(),
        };
        let invoice = base::InputInvoice::InputInvoiceStarGiftPrepaidUpgrade(Box::new(invoice));
        self.pay_in_stars_up_to(invoice, star_count).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::{client, gift_form, paid, payment};
    use crate::errors::Error;
    use crate::raw::{base, types};

    fn invoice() -> base::InputInvoice {
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        let invoice = types::InputInvoiceStarGiftPrepaidUpgrade { peer, hash: "h1".into() };
        base::InputInvoice::InputInvoiceStarGiftPrepaidUpgrade(Box::new(invoice))
    }

    #[tokio::test]
    async fn pays_up_to_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 900), paid()]);
        client.buy_gift_upgrade(5, "h1", 900).await.unwrap();
        let [get, pay] = payment(invoice());
        assert_eq!(sent.try_recv().unwrap(), get);
        assert_eq!(sent.try_recv().unwrap(), pay);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_price_above_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 901)]);
        assert!(matches!(client.buy_gift_upgrade(5, "h1", 900).await, Err(Error::InvalidArgument(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(invoice())[0]);
        assert!(sent.try_recv().is_err(), "nothing may be paid above star_count");
    }

    #[tokio::test]
    async fn misuse_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        for (hash, stars) in [("h1", 0), ("h1", -1), ("", 900)] {
            let refused = client.buy_gift_upgrade(5, hash, stars).await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{hash:?} {stars}");
        }
        // An unknown owner would need a lookup, so a bad cap must be refused before it.
        assert!(matches!(client.buy_gift_upgrade("someone", "h1", 0).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
