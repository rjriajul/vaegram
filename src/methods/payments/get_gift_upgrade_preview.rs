use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::GiftUpgradePreview;

impl Client {
    pub async fn get_gift_upgrade_preview(&self, gift_id: i64) -> Result<GiftUpgradePreview> {
        let request = functions::payments::GetStarGiftUpgradePreview { gift_id };
        Ok(GiftUpgradePreview::parse(&self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::GiftUpgradePrice;

    fn price(date: i32, upgrade_stars: i64) -> base::StarGiftUpgradePrice {
        base::StarGiftUpgradePrice::StarGiftUpgradePrice(Box::new(types::StarGiftUpgradePrice { date, upgrade_stars }))
    }

    #[tokio::test]
    async fn keeps_the_price_order() {
        let backdrop = types::StarGiftAttributeBackdrop {
            name: "Night".into(),
            backdrop_id: 1,
            center_color: 1,
            edge_color: 2,
            pattern_color: 3,
            text_color: 4,
            rarity: base::StarGiftAttributeRarity::StarGiftAttributeRarityEpic,
        };
        let preview = types::payments::StarGiftUpgradePreview {
            sample_attributes: vec![base::StarGiftAttribute::StarGiftAttributeBackdrop(Box::new(backdrop))],
            prices: vec![price(30, 900), price(10, 500), price(20, 700)],
            next_prices: vec![price(40, 1000)],
        };
        let reply = base::payments::StarGiftUpgradePreview::StarGiftUpgradePreview(Box::new(preview));
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let preview = client.get_gift_upgrade_preview(42).await.unwrap();
        let at = |date, star_count| GiftUpgradePrice { date, star_count };
        assert_eq!(preview.prices, [at(30, 900), at(10, 500), at(20, 700)]);
        assert_eq!(preview.next_prices, [at(40, 1000)]);
        assert_eq!(preview.variants.backdrops.len(), 1, "the variants come from the same parser");
        let request = functions::payments::GetStarGiftUpgradePreview { gift_id: 42 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
