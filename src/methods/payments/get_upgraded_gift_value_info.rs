use super::send_resold_gift::gift_slug;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::UpgradedGiftValueInfo;

impl Client {
    /// `gift` is the gift's name or its `t.me/nft/<name>` link.
    pub async fn get_upgraded_gift_value_info(&self, gift: &str) -> Result<UpgradedGiftValueInfo> {
        let request = functions::payments::GetUniqueStarGiftValueInfo { slug: gift_slug(gift)?.into() };
        Ok(UpgradedGiftValueInfo::parse(self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::UpgradedGiftValueInfo;

    fn info() -> types::payments::UniqueStarGiftValueInfo {
        types::payments::UniqueStarGiftValueInfo {
            last_sale_on_fragment: true,
            value_is_average: true,
            currency: "USD".into(),
            value: 1500,
            initial_sale_date: 10,
            initial_sale_stars: 500,
            initial_sale_price: 700,
            last_sale_date: Some(20),
            last_sale_price: Some(1400),
            floor_price: Some(1200),
            average_price: Some(1300),
            listed_count: Some(4),
            fragment_listed_count: Some(5),
            fragment_listed_url: Some("https://fragment.com/gift/x".into()),
        }
    }

    fn reply(info: types::payments::UniqueStarGiftValueInfo) -> Vec<u8> {
        base::payments::UniqueStarGiftValueInfo::UniqueStarGiftValueInfo(Box::new(info)).to_bytes()
    }

    #[tokio::test]
    async fn maps_every_field() {
        let (client, mut sent) = Client::scripted(vec![Ok(reply(info()))]);
        let info = client.get_upgraded_gift_value_info("t.me/nft/PlushPepe-3").await.unwrap();
        let expected = UpgradedGiftValueInfo {
            currency: "USD".into(),
            value: 1500,
            is_value_average: true,
            initial_sale_date: 10,
            initial_sale_star_count: 500,
            initial_sale_price: 700,
            last_sale_date: Some(20),
            last_sale_price: Some(1400),
            is_last_sale_on_fragment: true,
            minimum_price: Some(1200),
            average_sale_price: Some(1300),
            telegram_listed_gift_count: Some(4),
            fragment_listed_gift_count: Some(5),
            fragment_url: Some("https://fragment.com/gift/x".into()),
        };
        assert_eq!(info, expected);
        let request = functions::payments::GetUniqueStarGiftValueInfo { slug: "PlushPepe-3".into() };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn absent_flags_are_none() {
        let bare = types::payments::UniqueStarGiftValueInfo {
            last_sale_on_fragment: false,
            value_is_average: false,
            last_sale_date: None,
            last_sale_price: None,
            floor_price: None,
            average_price: None,
            listed_count: None,
            fragment_listed_count: None,
            fragment_listed_url: None,
            ..info()
        };
        let (client, _sent) = Client::scripted(vec![Ok(reply(bare))]);
        let info = client.get_upgraded_gift_value_info("PlushPepe-3").await.unwrap();
        assert!(!info.is_value_average && !info.is_last_sale_on_fragment);
        assert_eq!(
            (info.last_sale_date, info.last_sale_price, info.minimum_price, info.average_sale_price),
            (None, None, None, None)
        );
        assert_eq!(
            (info.telegram_listed_gift_count, info.fragment_listed_gift_count, info.fragment_url),
            (None, None, None)
        );
    }

    #[tokio::test]
    async fn a_link_without_a_name_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.get_upgraded_gift_value_info("").await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
