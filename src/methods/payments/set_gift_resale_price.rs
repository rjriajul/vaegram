use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::StarAmount;

impl Client {
    /// Lists an upgraded gift for sale at `price`, or takes it off sale with `None`.
    pub async fn set_gift_resale_price(&self, owned_gift_id: &str, price: Option<StarAmount>) -> Result<()> {
        if owned_gift_id.is_empty() {
            return Err(Error::InvalidArgument("owned_gift_id is empty"));
        }
        let resell_amount = match price {
            Some(price) => stars_amount(price)?,
            None => base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount: 0, nanos: 0 })),
        };
        let stargift = self.input_saved_gift(owned_gift_id).await?;
        self.invoke(&functions::payments::UpdateStarGiftPrice { stargift, resell_amount }).await?;
        Ok(())
    }
}

pub(super) fn stars_amount(price: StarAmount) -> Result<base::StarsAmount> {
    if price.amount <= 0 {
        return Err(Error::InvalidArgument("the price must be positive"));
    }
    Ok(if price.ton {
        base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: price.amount })
    } else {
        base::StarsAmount::StarsAmount(Box::new(types::StarsAmount {
            amount: price.amount,
            nanos: price.nanostar_amount,
        }))
    })
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::StarAmount;

    fn request(resell_amount: base::StarsAmount) -> Vec<u8> {
        let stargift = base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id: 9 });
        functions::payments::UpdateStarGiftPrice { stargift, resell_amount }.to_bytes()
    }

    fn stars(amount: i64) -> base::StarsAmount {
        base::StarsAmount::StarsAmount(Box::new(types::StarsAmount { amount, nanos: 0 }))
    }

    #[tokio::test]
    async fn lists_in_stars_or_ton_and_unlists() {
        let replies = (0..3).map(|_| Ok(base::Updates::UpdatesTooLong.to_bytes())).collect();
        let (client, mut sent) = Client::scripted(replies);
        let in_stars = StarAmount { amount: 500, nanostar_amount: 0, ton: false };
        client.set_gift_resale_price("9", Some(in_stars)).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), request(stars(500)));

        let in_ton = StarAmount { amount: 2_000_000_000, nanostar_amount: 0, ton: true };
        client.set_gift_resale_price("9", Some(in_ton)).await.unwrap();
        let ton = base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 2_000_000_000 });
        assert_eq!(sent.try_recv().unwrap(), request(ton));

        client.set_gift_resale_price("9", None).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), request(stars(0)));
    }

    #[tokio::test]
    async fn a_price_of_nothing_or_no_gift_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        for amount in [0, -5] {
            for ton in [false, true] {
                let price = StarAmount { amount, nanostar_amount: 0, ton };
                let refused = client.set_gift_resale_price("9", Some(price)).await;
                assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{amount} {ton}");
            }
        }
        assert!(matches!(client.set_gift_resale_price("", None).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
