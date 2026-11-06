use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::{ConvertGiftToStars, SendGift, SendGiftPurchaseOffer, SetEmojiStatus, TransferGift, UpgradeGift};
use crate::types::{
    Gift, GiftAuctionState, GiftInfo, OwnedGift, OwnedGiftRegular, OwnedGiftUnique, PeerRef, StarAmount, UniqueGift,
    UniqueGiftInfo,
};

// A gift that doesn't say where it is kept can't be addressed.
fn owned_id(id: &Option<String>) -> Result<&str> {
    id.as_deref().ok_or(Error::InvalidArgument("the gift has no owned_gift_id"))
}

impl Gift {
    /// Sends this gift to `chat_id`.
    pub fn send<'a>(&self, client: &'a Client, chat_id: impl Into<PeerRef>) -> SendGift<'a> {
        client.send_gift(chat_id, self.id)
    }

    /// Fetches the state of this gift's auction.
    pub async fn get_auction_state(&self, client: &Client) -> Result<GiftAuctionState> {
        client.get_gift_auction_state(self.id).await
    }
}

impl UniqueGift {
    /// Shows this gift as the account's emoji status. The account must own it.
    pub fn wear<'a>(&self, client: &'a Client) -> SetEmojiStatus<'a> {
        client.set_emoji_status(None).collectible_id(self.id)
    }

    /// Transfers this gift, which the account must own.
    pub fn transfer<'a>(&self, client: &'a Client, to: impl Into<PeerRef>) -> TransferGift<'a> {
        client.transfer_gift(self.name.clone(), to)
    }

    /// Buys this gift from its owner for `new_owner`, paying nothing above `price`.
    pub async fn buy(&self, client: &Client, new_owner: impl Into<PeerRef>, price: StarAmount) -> Result<()> {
        client.send_resold_gift(&self.name, new_owner, price).await
    }

    /// Offers `price` to the owner, open for `duration` seconds. Awaiting fails with `Error::InvalidArgument` when
    /// [`can_send_purchase_offer`] is false: the gift has no known owner or doesn't take offers.
    ///
    /// [`can_send_purchase_offer`]: UniqueGift::can_send_purchase_offer
    pub fn send_purchase_offer<'a>(
        &'a self,
        client: &'a Client,
        price: StarAmount,
        duration: i32,
    ) -> SendGiftPurchaseOffer<'a> {
        let owner = self.owner.as_ref().filter(|_| self.can_send_purchase_offer()).map(|owner| owner.id.into());
        client.offer_gift(owner, &self.name, price, duration)
    }
}

impl OwnedGift {
    fn id(&self) -> Result<&str> {
        match self {
            OwnedGift::Regular(g) => owned_id(&g.owned_gift_id),
            OwnedGift::Unique(g) => owned_id(&g.owned_gift_id),
        }
    }

    /// Shows the gift on the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn show(&self, client: &Client) -> Result<()> {
        client.show_gift(self.id()?).await
    }

    /// Hides the gift from the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn hide(&self, client: &Client) -> Result<()> {
        client.hide_gift(self.id()?).await
    }
}

impl OwnedGiftRegular {
    /// Shows the gift on the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn show(&self, client: &Client) -> Result<()> {
        client.show_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Hides the gift from the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn hide(&self, client: &Client) -> Result<()> {
        client.hide_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Builds a request that converts the gift to Stars and gives it up.
    ///
    /// Awaiting fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub fn convert<'a>(&self, client: &'a Client) -> ConvertGiftToStars<'a> {
        client.convert_gift_to_stars(self.owned_gift_id.clone().unwrap_or_default())
    }

    /// Builds a request that upgrades the gift to a unique gift.
    ///
    /// Awaiting fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub fn upgrade<'a>(&self, client: &'a Client) -> UpgradeGift<'a> {
        client.upgrade_gift(self.owned_gift_id.clone().unwrap_or_default())
    }
}

impl OwnedGiftUnique {
    /// Shows the gift on the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn show(&self, client: &Client) -> Result<()> {
        client.show_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Hides the gift from the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn hide(&self, client: &Client) -> Result<()> {
        client.hide_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Transfers this gift. Without an `owned_gift_id`, the request names the gift by its slug.
    pub fn transfer<'a>(&self, client: &'a Client, to: impl Into<PeerRef>) -> TransferGift<'a> {
        client.transfer_gift(self.owned_gift_id.clone().unwrap_or_else(|| self.gift.name.clone()), to)
    }
}

impl GiftInfo {
    /// Shows the gift on the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn show(&self, client: &Client) -> Result<()> {
        client.show_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Hides the gift from the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn hide(&self, client: &Client) -> Result<()> {
        client.hide_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Builds a request that converts the gift to Stars and gives it up.
    ///
    /// Awaiting fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub fn convert<'a>(&self, client: &'a Client) -> ConvertGiftToStars<'a> {
        client.convert_gift_to_stars(self.owned_gift_id.clone().unwrap_or_default())
    }

    /// Builds a request that upgrades the gift to a unique gift.
    ///
    /// Awaiting fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub fn upgrade<'a>(&self, client: &'a Client) -> UpgradeGift<'a> {
        client.upgrade_gift(self.owned_gift_id.clone().unwrap_or_default())
    }
}

impl UniqueGiftInfo {
    /// Shows the gift on the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn show(&self, client: &Client) -> Result<()> {
        client.show_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Hides the gift from the profile. Fails with `Error::InvalidArgument` if `owned_gift_id` is `None`.
    pub async fn hide(&self, client: &Client) -> Result<()> {
        client.hide_gift(owned_id(&self.owned_gift_id)?).await
    }

    /// Transfers this gift. Without an `owned_gift_id`, the request names the gift by its slug.
    pub fn transfer<'a>(&self, client: &'a Client, to: impl Into<PeerRef>) -> TransferGift<'a> {
        client.transfer_gift(self.owned_gift_id.clone().unwrap_or_else(|| self.gift.name.clone()), to)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::{OwnedGift, OwnedGiftUnique, StarAmount, UniqueGift};

    #[tokio::test]
    async fn wear_sets_the_collectible_status() {
        let gift = UniqueGift {
            id: 11,
            gift_id: 2,
            base_name: "Plush Pepe".into(),
            name: "PlushPepe-3".into(),
            number: 3,
            model: None,
            symbol: None,
            backdrop: None,
            is_premium: false,
            is_burned: false,
            publisher_chat: None,
            ..UniqueGift::empty()
        };
        let (client, mut sent) = Client::recording();
        assert!(gift.wear(&client).await.is_err());
        let collectible = types::InputEmojiStatusCollectible { collectible_id: 11, until: None };
        let emoji_status = base::EmojiStatus::InputEmojiStatusCollectible(Box::new(collectible));
        assert_eq!(sent.try_recv().unwrap(), functions::account::UpdateEmojiStatus { emoji_status }.to_bytes());
    }

    fn unique() -> UniqueGift {
        UniqueGift { id: 11, gift_id: 2, name: "PlushPepe-3".into(), ..UniqueGift::empty() }
    }

    fn owner_client() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        client.0.peers.lock().unwrap().insert(5, peer);
        (client, sent)
    }

    fn constructor(body: &[u8]) -> u32 {
        u32::from_le_bytes(body[..4].try_into().unwrap())
    }

    fn slug() -> base::InputSavedStarGift {
        base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
            slug: "PlushPepe-3".into(),
        }))
    }

    #[tokio::test]
    async fn each_gift_helper_sends_its_request() {
        let (client, mut sent) = owner_client();
        let gift = unique();
        let _ = gift.transfer(&client, 5_i64).await;
        let transfer = functions::payments::TransferStarGift {
            stargift: slug(),
            to_id: base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 })),
        };
        assert_eq!(sent.try_recv().unwrap(), transfer.to_bytes());

        let _ = gift.buy(&client, 5_i64, StarAmount::new(9)).await;
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::GetPaymentForm::CONSTRUCTOR_ID);

        let owned = OwnedGiftUnique {
            gift: gift.clone(),
            owned_gift_id: Some("9".into()),
            sender_user: None,
            send_date: 0,
            is_saved: true,
            can_be_transferred: true,
            transfer_star_count: None,
            next_transfer_date: None,
            drop_original_details_star_count: None,
        };
        let _ = owned.show(&client).await;
        let show = functions::payments::SaveStarGift {
            unsave: false,
            stargift: base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id: 9 }),
        };
        assert_eq!(sent.try_recv().unwrap(), show.to_bytes());
        let _ = OwnedGift::Unique(Box::new(owned.clone())).hide(&client).await;
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::SaveStarGift::CONSTRUCTOR_ID);
        // Without an id the unique gift is addressed by its name.
        let nameless = OwnedGiftUnique { owned_gift_id: None, ..owned };
        let _ = nameless.transfer(&client, 5_i64).await;
        assert_eq!(sent.try_recv().unwrap(), transfer.to_bytes());
    }

    #[tokio::test]
    async fn purchase_offers_need_an_owner() {
        let (client, mut sent) = owner_client();
        let price = StarAmount::new(9);
        assert!(matches!(unique().send_purchase_offer(&client, price, 60).await, Err(Error::InvalidArgument(_))));
        // An owner alone is not enough: the gift must take offers.
        let owner =
            crate::types::Chat::parse(&crate::utils::get_peer(5), &crate::dispatcher::Entities::default(), true);
        let no_minimum = UniqueGift { owner: Some(owner.clone()), ..unique() };
        assert!(matches!(no_minimum.send_purchase_offer(&client, price, 60).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
        let open = UniqueGift { owner: Some(owner), minimum_offer_star_count: Some(10), ..unique() };
        let _ = open.send_purchase_offer(&client, price, 60).await;
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::SendStarGiftOffer::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn owned_helpers_refuse_without_an_id() {
        let (client, mut sent) = owner_client();
        let owned = OwnedGiftUnique {
            gift: unique(),
            owned_gift_id: None,
            sender_user: None,
            send_date: 0,
            is_saved: true,
            can_be_transferred: false,
            transfer_star_count: None,
            next_transfer_date: None,
            drop_original_details_star_count: None,
        };
        assert!(matches!(owned.show(&client).await, Err(Error::InvalidArgument(_))));
        assert!(matches!(owned.hide(&client).await, Err(Error::InvalidArgument(_))));
        assert!(matches!(OwnedGift::Unique(Box::new(owned)).show(&client).await, Err(Error::InvalidArgument(_))));
        // convert and upgrade pass a missing id on as an empty one, which the request refuses on await.
        assert!(matches!(client.convert_gift_to_stars("").await, Err(Error::InvalidArgument(_))));
        assert!(matches!(client.upgrade_gift("").await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err(), "no request without an id");
    }
}
