use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, PremiumGiftCodeOption};

impl Client {
    /// Lists the Premium gift-code packages that can be bought.
    ///
    /// Pass the channel the codes are bought for as `chat_id` to get the packages for giving away from it; the
    /// codes then boost the channel. `None` lists the general packages.
    pub async fn get_premium_gift_code_options(&self, chat_id: Option<PeerRef>) -> Result<Vec<PremiumGiftCodeOption>> {
        let boost_peer = match chat_id {
            Some(chat_id) => Some(self.resolve_peer(chat_id).await?),
            None => None,
        };
        let options = self.invoke(&functions::payments::GetPremiumGiftCodeOptions { boost_peer }).await?;
        Ok(options.into_iter().map(PremiumGiftCodeOption::parse).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::payments;
    use crate::raw::{Serializable, base, types};
    use crate::types::{PeerRef, PremiumGiftCodeOption};

    #[tokio::test]
    async fn lists_the_packages_with_and_without_a_channel() {
        let raw = types::PremiumGiftCodeOption {
            users: 5,
            months: 3,
            store_product: Some("premium_3".into()),
            store_quantity: Some(5),
            currency: "USD".into(),
            amount: 1499,
        };
        let reply = vec![base::PremiumGiftCodeOption::PremiumGiftCodeOption(Box::new(raw))];
        let (client, mut sent) = Client::scripted_raw(vec![Ok(reply.to_bytes()), Ok(reply.to_bytes())]);
        let expected = vec![PremiumGiftCodeOption {
            user_count: 5,
            months: 3,
            currency: "USD".into(),
            amount: 1499,
            store_product_id: Some("premium_3".into()),
            store_quantity: Some(5),
        }];

        assert_eq!(client.get_premium_gift_code_options(None).await.unwrap(), expected);
        let request = payments::GetPremiumGiftCodeOptions { boost_peer: None };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());

        assert_eq!(client.get_premium_gift_code_options(Some(PeerRef::Me)).await.unwrap(), expected);
        let request = payments::GetPremiumGiftCodeOptions { boost_peer: Some(base::InputPeer::InputPeerSelf) };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
