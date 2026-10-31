use super::send_resold_gift::gift_slug;
use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::UniqueGift;

impl Client {
    /// `gift` is the gift's name or its `t.me/nft/<name>` link.
    pub async fn get_upgraded_gift(&self, gift: &str) -> Result<UniqueGift> {
        let request = functions::payments::GetUniqueStarGift { slug: gift_slug(gift)?.into() };
        let base::payments::UniqueStarGift::UniqueStarGift(reply) = self.invoke(&request).await?;
        let reply = *reply;
        let entities = Entities::new(self, reply.users, reply.chats);
        match reply.gift {
            base::StarGift::StarGiftUnique(gift) => Ok(UniqueGift::parse(&gift, &entities)),
            base::StarGift::StarGift(_) => Err(Error::Unexpected("payments.getUniqueStarGift returned a regular gift")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    fn unique() -> base::StarGift {
        base::StarGift::StarGiftUnique(Box::new(types::StarGiftUnique {
            require_premium: false,
            resale_ton_only: false,
            theme_available: false,
            burned: false,
            crafted: false,
            id: 1,
            gift_id: 2,
            title: "Plush Pepe".into(),
            slug: "PlushPepe-3".into(),
            num: 3,
            owner_id: None,
            owner_name: None,
            owner_address: None,
            attributes: Vec::new(),
            availability_issued: 10,
            availability_total: 100,
            gift_address: None,
            resell_amount: None,
            released_by: None,
            value_amount: None,
            value_currency: None,
            value_usd_amount: None,
            theme_peer: None,
            peer_color: None,
            host_id: None,
            offer_min_stars: None,
            craft_chance_permille: None,
        }))
    }

    fn reply(gift: base::StarGift) -> Vec<u8> {
        let reply = types::payments::UniqueStarGift { gift, chats: Vec::new(), users: Vec::new() };
        base::payments::UniqueStarGift::UniqueStarGift(Box::new(reply)).to_bytes()
    }

    #[tokio::test]
    async fn reads_a_gift_by_its_link() {
        let (client, mut sent) = Client::scripted(vec![Ok(reply(unique()))]);
        let gift = client.get_upgraded_gift("https://t.me/nft/PlushPepe-3").await.unwrap();
        assert_eq!((gift.name.as_str(), gift.number, gift.gift_id), ("PlushPepe-3", 3, 2));
        let request = functions::payments::GetUniqueStarGift { slug: "PlushPepe-3".into() };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn a_link_without_a_name_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted(Vec::new());
        assert!(matches!(client.get_upgraded_gift("t.me/nft/").await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_regular_gift_is_unexpected() {
        let regular = types::StarGift {
            limited: false,
            sold_out: false,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: false,
            id: 1,
            sticker: base::Document::DocumentEmpty(types::DocumentEmpty { id: 1 }),
            stars: 50,
            availability_remains: None,
            availability_total: None,
            availability_resale: None,
            convert_stars: 40,
            first_sale_date: None,
            last_sale_date: None,
            upgrade_stars: None,
            resell_min_stars: None,
            title: None,
            released_by: None,
            per_user_total: None,
            per_user_remains: None,
            locked_until_date: None,
            auction_slug: None,
            gifts_per_round: None,
            auction_start_date: None,
            upgrade_variants: None,
            background: None,
        };
        let (client, _sent) = Client::scripted(vec![Ok(reply(base::StarGift::StarGift(Box::new(regular))))]);
        assert!(matches!(client.get_upgraded_gift("PlushPepe-3").await, Err(Error::Unexpected(_))));
    }
}
