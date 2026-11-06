use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::messages_and_media::text_with_entities;
use crate::types::{Gift, MessageEntity, UniqueGift, User};

/// A gift on a user's or channel's profile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OwnedGift {
    /// A gift that isn't upgraded.
    Regular(Box<OwnedGiftRegular>),
    /// A gift upgraded to a unique one.
    Unique(Box<OwnedGiftUnique>),
}

/// A gift on a profile that hasn't been upgraded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OwnedGiftRegular {
    /// The gift.
    pub gift: Gift,
    /// Id to address the gift with. `None` if Telegram sends neither a message id nor a saved id.
    pub owned_gift_id: Option<String>,
    /// The user who sent the gift. `None` if Telegram sends no sender or the sender isn't a user.
    pub sender_user: Option<User>,
    /// Unix time the gift was sent.
    pub send_date: i32,
    /// The text the sender attached. `None` if there is none.
    pub text: Option<String>,
    /// Formatting entities of `text`.
    pub entities: Option<Vec<MessageEntity>>,
    /// Whether the sender's name is hidden.
    pub is_private: bool,
    /// Whether the gift is shown on the profile.
    pub is_saved: bool,
    /// Whether the gift can be upgraded to a unique gift.
    pub can_be_upgraded: bool,
    /// Whether the gift was refunded.
    pub was_refunded: bool,
    /// Stars the owner gets for converting the gift.
    pub convert_star_count: Option<i64>,
    /// Stars paid in advance for the upgrade.
    pub prepaid_upgrade_star_count: Option<i64>,
    /// Whether the upgrade is paid separately from the gift.
    pub is_upgrade_separate: bool,
    /// Serial number of the gift.
    pub unique_gift_number: Option<i32>,
    /// Lets anyone pay for this gift's upgrade with [`Client::buy_gift_upgrade`].
    pub prepaid_upgrade_hash: Option<String>,
}

/// A unique gift on a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct OwnedGiftUnique {
    /// The gift.
    pub gift: UniqueGift,
    /// Id to address the gift with. `None` if Telegram sends neither a message id nor a saved id.
    pub owned_gift_id: Option<String>,
    /// The user who sent the gift. `None` if Telegram sends no sender or the sender isn't a user.
    pub sender_user: Option<User>,
    /// Unix time the gift was sent.
    pub send_date: i32,
    /// Whether the gift is shown on the profile.
    pub is_saved: bool,
    /// `true` if Telegram sent a transfer price or a transfer date for the gift.
    pub can_be_transferred: bool,
    /// Stars a transfer costs.
    pub transfer_star_count: Option<i64>,
    /// Unix time the gift can be transferred from.
    pub next_transfer_date: Option<i32>,
    /// Stars it costs to remove the original details.
    pub drop_original_details_star_count: Option<i64>,
}

impl OwnedGift {
    pub(crate) fn parse(raw: &types::SavedStarGift, owner_id: i64, entities: &Entities) -> Option<OwnedGift> {
        let owned_gift_id = owned_gift_id(owner_id, raw.msg_id, raw.saved_id);
        let sender_user = match &raw.from_id {
            Some(base::Peer::PeerUser(p)) => Some(User::parse(p.user_id, entities)),
            _ => None,
        };
        Some(match &raw.gift {
            base::StarGift::StarGift(g) => {
                let (text, text_entities) = split_text(raw.message.as_ref(), entities);
                OwnedGift::Regular(Box::new(OwnedGiftRegular {
                    gift: Gift::parse(g, entities)?,
                    owned_gift_id,
                    sender_user,
                    send_date: raw.date,
                    text,
                    entities: text_entities,
                    is_private: raw.name_hidden,
                    is_saved: !raw.unsaved,
                    can_be_upgraded: raw.can_upgrade,
                    was_refunded: raw.refunded,
                    convert_star_count: raw.convert_stars,
                    prepaid_upgrade_star_count: raw.upgrade_stars,
                    is_upgrade_separate: raw.upgrade_separate,
                    unique_gift_number: raw.gift_num,
                    prepaid_upgrade_hash: raw.prepaid_upgrade_hash.clone(),
                }))
            }
            base::StarGift::StarGiftUnique(g) => OwnedGift::Unique(Box::new(OwnedGiftUnique {
                gift: UniqueGift::parse(g, entities),
                owned_gift_id,
                sender_user,
                send_date: raw.date,
                is_saved: !raw.unsaved,
                can_be_transferred: raw.transfer_stars.is_some() || raw.can_transfer_at.is_some(),
                transfer_star_count: raw.transfer_stars,
                next_transfer_date: raw.can_transfer_at,
                drop_original_details_star_count: raw.drop_original_details_stars,
            })),
        })
    }
}

pub(crate) fn split_text(
    message: Option<&base::TextWithEntities>,
    entities: &Entities,
) -> (Option<String>, Option<Vec<MessageEntity>>) {
    match message.map(|m| text_with_entities(m, entities)) {
        Some((text, text_entities)) => (Some(text), text_entities),
        None => (None, None),
    }
}

// "<msg_id>" for gifts kept by users, "<chat_id>_<saved_id>" for channel gifts; anything else is a unique gift's slug.
pub(crate) fn owned_gift_id(owner_id: i64, msg_id: Option<i32>, saved_id: Option<i64>) -> Option<String> {
    match (saved_id, msg_id) {
        (Some(saved_id), _) => Some(format!("{owner_id}_{saved_id}")),
        (None, Some(msg_id)) => Some(msg_id.to_string()),
        (None, None) => None,
    }
}

enum OwnedGiftRef<'a> {
    Message(i32),
    Chat(i64, i64),
    Slug(&'a str),
}

fn parse_owned_gift_id(id: &str) -> OwnedGiftRef<'_> {
    if let Ok(msg_id) = id.parse() {
        return OwnedGiftRef::Message(msg_id);
    }
    if let Some((Ok(chat_id), Ok(saved_id))) = id.split_once('_').map(|(c, s)| (c.parse(), s.parse())) {
        return OwnedGiftRef::Chat(chat_id, saved_id);
    }
    OwnedGiftRef::Slug(id)
}

impl Client {
    pub(crate) async fn input_saved_gift(&self, owned_gift_id: &str) -> Result<base::InputSavedStarGift> {
        Ok(match parse_owned_gift_id(owned_gift_id) {
            OwnedGiftRef::Message(msg_id) => {
                base::InputSavedStarGift::InputSavedStarGiftUser(types::InputSavedStarGiftUser { msg_id })
            }
            OwnedGiftRef::Chat(chat_id, saved_id) => {
                let peer = self.resolve_peer(chat_id).await?;
                base::InputSavedStarGift::InputSavedStarGiftChat(Box::new(types::InputSavedStarGiftChat {
                    peer,
                    saved_id,
                }))
            }
            OwnedGiftRef::Slug(slug) => {
                base::InputSavedStarGift::InputSavedStarGiftSlug(Box::new(types::InputSavedStarGiftSlug {
                    slug: slug.to_owned(),
                }))
            }
        })
    }

    pub(crate) async fn input_saved_gifts(
        &self,
        owned_gift_ids: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Vec<base::InputSavedStarGift>> {
        let owned_gift_ids = owned_gift_ids.into_iter();
        let mut gifts = Vec::with_capacity(owned_gift_ids.size_hint().0);
        for id in owned_gift_ids {
            gifts.push(self.input_saved_gift(id.as_ref()).await?);
        }
        Ok(gifts)
    }
}

#[cfg(test)]
mod tests {
    use super::{OwnedGift, OwnedGiftRef, owned_gift_id, parse_owned_gift_id};
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::media::tests::document_media;

    #[test]
    fn owned_gift_ids_round_trip() {
        let chat = owned_gift_id(-1000000000005, Some(9), Some(42)).unwrap();
        assert_eq!(chat, "-1000000000005_42");
        assert!(matches!(parse_owned_gift_id(&chat), OwnedGiftRef::Chat(-1000000000005, 42)));

        let user = owned_gift_id(7, Some(9), None).unwrap();
        assert!(matches!(parse_owned_gift_id(&user), OwnedGiftRef::Message(9)));

        assert_eq!(owned_gift_id(7, None, None), None);
        assert!(matches!(parse_owned_gift_id("PlushPepe-12"), OwnedGiftRef::Slug("PlushPepe-12")));
    }

    fn regular_gift() -> base::StarGift {
        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let base::MessageMedia::MessageMediaDocument(media) = document_media("image/webp", vec![sticker]) else {
            panic!("expected a document")
        };
        base::StarGift::StarGift(Box::new(types::StarGift {
            limited: false,
            sold_out: false,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: false,
            id: 1,
            sticker: media.document.expect("a sticker"),
            stars: 50,
            availability_remains: None,
            availability_total: None,
            availability_resale: None,
            convert_stars: 40,
            first_sale_date: None,
            last_sale_date: None,
            upgrade_stars: Some(25),
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
        }))
    }

    fn unique_gift() -> base::StarGift {
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

    fn saved(gift: base::StarGift) -> types::SavedStarGift {
        types::SavedStarGift {
            name_hidden: false,
            unsaved: false,
            refunded: false,
            can_upgrade: false,
            pinned_to_top: false,
            upgrade_separate: false,
            from_id: None,
            date: 1,
            gift,
            message: None,
            msg_id: Some(9),
            saved_id: None,
            convert_stars: None,
            upgrade_stars: None,
            can_export_at: None,
            transfer_stars: None,
            can_transfer_at: None,
            can_resell_at: None,
            collection_id: None,
            prepaid_upgrade_hash: Some("h1".into()),
            drop_original_details_stars: Some(250),
            gift_num: None,
            can_craft_at: None,
        }
    }

    #[test]
    fn maps_upgrade_hash_and_detail_drop_price() {
        let entities = Entities::default();
        let Some(OwnedGift::Regular(regular)) = OwnedGift::parse(&saved(regular_gift()), 7, &entities) else {
            panic!("expected a regular gift")
        };
        assert_eq!(regular.prepaid_upgrade_hash.as_deref(), Some("h1"));
        let Some(OwnedGift::Unique(unique)) = OwnedGift::parse(&saved(unique_gift()), 7, &entities) else {
            panic!("expected a unique gift")
        };
        assert_eq!(unique.drop_original_details_star_count, Some(250));

        let bare = types::SavedStarGift {
            prepaid_upgrade_hash: None,
            drop_original_details_stars: None,
            ..saved(regular_gift())
        };
        let Some(OwnedGift::Regular(regular)) = OwnedGift::parse(&bare, 7, &entities) else {
            panic!("expected a regular gift")
        };
        assert_eq!(regular.prepaid_upgrade_hash, None);
    }
}
