//! Gifts: the shop, the gifts on a profile, unique gifts, and what an owner does with a gift; and what the bot
//! pays for in Stars, which TL asks as a payment form and then pays.

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::stickers::{Sticker, sticker_document};
use super::{done, no_updates, parse};
use crate::bot_api::bridge::{Transport, unsupported};
use crate::bot_api::convert::{self, Peers, int};
use crate::bot_api::raw::types as bot;
use crate::errors::{Error, Result};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::utils::{get_peer, input_peer_id};

const STAR_GIFTS: u32 = <f::payments::GetStarGifts as Identifiable>::CONSTRUCTOR_ID;
const SAVED_GIFTS: u32 = <f::payments::GetSavedStarGifts as Identifiable>::CONSTRUCTOR_ID;
const UNIQUE_GIFT: u32 = <f::payments::GetUniqueStarGift as Identifiable>::CONSTRUCTOR_ID;
const CONVERT: u32 = <f::payments::ConvertStarGift as Identifiable>::CONSTRUCTOR_ID;
const UPGRADE: u32 = <f::payments::UpgradeStarGift as Identifiable>::CONSTRUCTOR_ID;
const TRANSFER: u32 = <f::payments::TransferStarGift as Identifiable>::CONSTRUCTOR_ID;
const DELETE: u32 = <f::payments::DeleteStarGift as Identifiable>::CONSTRUCTOR_ID;
const PAYMENT_FORM: u32 = <f::payments::GetPaymentForm as Identifiable>::CONSTRUCTOR_ID;
const SEND_STARS_FORM: u32 = <f::payments::SendStarsForm as Identifiable>::CONSTRUCTOR_ID;

/// What Premium for 3, 6 and 12 months costs in Stars, which `giftPremiumSubscription` wants stated. The Bot API
/// fixes these prices itself: "must be 1000 for 3 months, 1500 for 6 months, and 2500 for 12 months"
/// (<https://core.telegram.org/bots/api#giftpremiumsubscription>, checked 2026-11-27). TL's request names only the
/// months, so the price can't come from the caller.
const PREMIUM_PRICES: [(i32, i64); 3] = [(3, 1000), (6, 1500), (12, 2500)];

/// A Bot API `Gift`, with the fields TL has a place for.
#[derive(Deserialize)]
struct Gift {
    id: String,
    sticker: Sticker,
    #[serde(default)]
    star_count: i64,
    upgrade_star_count: Option<i64>,
    #[serde(default)]
    is_premium: bool,
    #[serde(default)]
    has_colors: bool,
    total_count: Option<i64>,
    remaining_count: Option<i64>,
    personal_total_count: Option<i64>,
    personal_remaining_count: Option<i64>,
    background: Option<Colors>,
    unique_gift_variant_count: Option<i64>,
    publisher_chat: Option<bot::Chat>,
}

/// The colors of a gift's background or a unique gift's backdrop.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Colors {
    center_color: i64,
    edge_color: i64,
    symbol_color: i64,
    text_color: i64,
}

/// A Bot API `UniqueGift`.
#[derive(Deserialize)]
struct UniqueGift {
    gift_id: String,
    #[serde(default)]
    base_name: String,
    name: String,
    #[serde(default)]
    number: i64,
    model: Option<Attribute>,
    symbol: Option<Attribute>,
    backdrop: Option<Backdrop>,
    #[serde(default)]
    is_premium: bool,
    #[serde(default)]
    is_burned: bool,
    publisher_chat: Option<bot::Chat>,
}

/// A unique gift's model or symbol.
#[derive(Deserialize)]
struct Attribute {
    name: String,
    sticker: Sticker,
    #[serde(default)]
    rarity_per_mille: i64,
}

#[derive(Deserialize)]
struct Backdrop {
    name: String,
    #[serde(default)]
    colors: Colors,
    #[serde(default)]
    rarity_per_mille: i64,
}

/// A Bot API `OwnedGift`, regular or unique.
#[derive(Deserialize)]
struct OwnedGift {
    #[serde(rename = "type")]
    kind: String,
    gift: Value,
    owned_gift_id: Option<String>,
    sender_user: Option<bot::User>,
    #[serde(default)]
    send_date: i64,
    text: Option<String>,
    entities: Option<Vec<bot::MessageEntity>>,
    #[serde(default)]
    is_private: bool,
    is_saved: Option<bool>,
    #[serde(default)]
    can_be_upgraded: bool,
    #[serde(default)]
    was_refunded: bool,
    convert_star_count: Option<i64>,
    prepaid_upgrade_star_count: Option<i64>,
    #[serde(default)]
    is_upgrade_separate: bool,
    unique_gift_number: Option<i64>,
    transfer_star_count: Option<i64>,
    next_transfer_date: Option<i64>,
}

#[derive(Deserialize)]
struct OwnedGifts {
    #[serde(default)]
    total_count: i64,
    #[serde(default)]
    gifts: Vec<OwnedGift>,
    next_offset: Option<String>,
}

impl Transport {
    /// Answers a gift request, made through the business connection `connection` if set; `None` when it isn't of
    /// this group.
    pub(super) async fn answer_gifts(&self, id: u32, body: &[u8], connection: Option<&str>) -> Option<Result<Vec<u8>>> {
        Some(match (id, connection) {
            (STAR_GIFTS, None) => self.available_gifts().await,
            (UNIQUE_GIFT, None) => self.unique_gift(body).await,
            (SAVED_GIFTS, _) => self.saved_gifts(body, connection).await,
            (PAYMENT_FORM, _) => payment_form(body),
            (SEND_STARS_FORM, _) => self.pay(body, connection).await,
            (CONVERT | UPGRADE | TRANSFER | DELETE, None) => {
                Err(unsupported("converting, upgrading, transferring or deleting the bot's own gifts"))
            }
            (CONVERT, Some(connection)) => self.convert_gift(body, connection).await,
            (UPGRADE, Some(connection)) => self.upgrade_gift(body, connection).await,
            (TRANSFER, Some(connection)) => self.transfer_gift(body, connection).await,
            (DELETE, Some(connection)) => self.delete_gift(body, connection).await,
            _ => return None,
        })
    }

    async fn available_gifts(&self) -> Result<Vec<u8>> {
        #[derive(Deserialize)]
        struct Gifts {
            #[serde(default)]
            gifts: Vec<Gift>,
        }
        let list: Gifts = parse(self.call("getAvailableGifts", json!({})).await?)?;
        let mut peers = Peers::new(self.me());
        let gifts = list.gifts.iter().filter_map(|gift| star_gift(gift, &mut peers)).collect();
        Ok(types::payments::StarGifts { hash: 0, gifts, chats: peers.chats, users: peers.users }.to_bytes())
    }

    async fn unique_gift(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::payments::GetUniqueStarGift::from_bytes(body)?;
        let gift: UniqueGift = parse(self.call("getUniqueGift", json!({ "name": r.slug })).await?)?;
        let mut peers = Peers::new(self.me());
        let gift = unique_star_gift(&gift, &mut peers)?;
        Ok(types::payments::UniqueStarGift { gift, chats: peers.chats, users: peers.users }.to_bytes())
    }

    /// `getBusinessAccountGifts` through a connection, else `getUserGifts` for a user (the bot itself for
    /// `inputPeerSelf`) and `getChatGifts` for a chat.
    async fn saved_gifts(&self, body: &[u8], connection: Option<&str>) -> Result<Vec<u8>> {
        let r = f::payments::GetSavedStarGifts::from_bytes(body)?;
        if r.collection_id.is_some() {
            return Err(unsupported("gift collections"));
        }
        let mut out = Map::new();
        let method = match connection {
            Some(connection) => {
                out.insert("business_connection_id".into(), connection.into());
                "getBusinessAccountGifts"
            }
            None => match input_peer_id(&r.peer, self.me()) {
                Some(user_id) if user_id > 0 => {
                    out.insert("user_id".into(), user_id.into());
                    "getUserGifts"
                }
                Some(chat_id) => {
                    out.insert("chat_id".into(), chat_id.into());
                    "getChatGifts"
                }
                None => return Err(Error::InvalidArgument("the request names no chat")),
            },
        };
        let saved = [("exclude_unsaved", r.exclude_unsaved), ("exclude_saved", r.exclude_saved)];
        if method == "getUserGifts" && saved.iter().any(|&(_, on)| on) {
            return Err(unsupported("leaving out a user's saved or unsaved gifts"));
        }
        let flags = [
            ("exclude_unlimited", r.exclude_unlimited),
            ("exclude_limited_upgradable", r.exclude_upgradable),
            ("exclude_limited_non_upgradable", r.exclude_unupgradable),
            ("exclude_unique", r.exclude_unique),
            ("sort_by_price", r.sort_by_value),
        ];
        for (key, _) in saved.into_iter().chain(flags).filter(|&(_, on)| on) {
            out.insert(key.into(), true.into());
        }
        if !r.offset.is_empty() {
            out.insert("offset".into(), r.offset.into());
        }
        if r.limit > 0 {
            out.insert("limit".into(), r.limit.into());
        }
        let page: OwnedGifts = parse(self.call(method, Value::Object(out)).await?)?;
        let mut peers = Peers::new(self.me());
        let mut gifts = Vec::with_capacity(page.gifts.len());
        for owned in page.gifts {
            if let Some(gift) = saved_star_gift(owned, &mut peers)? {
                gifts.push(base::SavedStarGift::SavedStarGift(Box::new(gift)));
            }
        }
        let mut answer = types::payments::SavedStarGifts::new(int(page.total_count), gifts, peers.chats, peers.users);
        answer.next_offset = page.next_offset.filter(|offset| !offset.is_empty());
        Ok(answer.to_bytes())
    }

    async fn convert_gift(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::payments::ConvertStarGift::from_bytes(body)?;
        let body = json!({ "business_connection_id": connection, "owned_gift_id": self.owned_gift_id(&r.stargift)? });
        self.call("convertGiftToStars", body).await?;
        Ok(done())
    }

    async fn upgrade_gift(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::payments::UpgradeStarGift::from_bytes(body)?;
        self.upgrade(connection, &r.stargift, r.keep_original_details).await?;
        Ok(no_updates())
    }

    async fn transfer_gift(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::payments::TransferStarGift::from_bytes(body)?;
        self.transfer(connection, &r.stargift, &r.to_id).await?;
        Ok(no_updates())
    }

    async fn delete_gift(&self, body: &[u8], connection: &str) -> Result<Vec<u8>> {
        let r = f::payments::DeleteStarGift::from_bytes(body)?;
        let body = json!({ "business_connection_id": connection, "owned_gift_id": self.owned_gift_id(&r.stargift)? });
        self.call("deleteBusinessAccountGift", body).await?;
        Ok(done())
    }

    /// `upgradeGift`. TL's invoice doesn't say what a paid upgrade costs, so no `star_count` is sent: Telegram takes
    /// a prepaid upgrade as is and may refuse one that isn't.
    async fn upgrade(
        &self,
        connection: &str,
        gift: &base::InputSavedStarGift,
        keep_original_details: bool,
    ) -> Result<()> {
        let mut out = Map::new();
        out.insert("business_connection_id".into(), connection.into());
        out.insert("owned_gift_id".into(), self.owned_gift_id(gift)?.into());
        if keep_original_details {
            out.insert("keep_original_details".into(), true.into());
        }
        self.call("upgradeGift", Value::Object(out)).await?;
        Ok(())
    }

    /// `transferGift`, with no `star_count` for the same reason as [`upgrade`](Self::upgrade).
    async fn transfer(&self, connection: &str, gift: &base::InputSavedStarGift, to: &base::InputPeer) -> Result<()> {
        let body = json!({
            "business_connection_id": connection,
            "owned_gift_id": self.owned_gift_id(gift)?,
            "new_owner_chat_id": self.chat_id(to)?,
        });
        self.call("transferGift", body).await?;
        Ok(())
    }

    /// The `owned_gift_id` an input gift was made from: a message id, `<chat id>_<saved id>`, or a unique gift's
    /// name.
    fn owned_gift_id(&self, gift: &base::InputSavedStarGift) -> Result<String> {
        Ok(match gift {
            base::InputSavedStarGift::InputSavedStarGiftUser(g) => g.msg_id.to_string(),
            base::InputSavedStarGift::InputSavedStarGiftChat(g) => format!("{}_{}", self.chat_id(&g.peer)?, g.saved_id),
            base::InputSavedStarGift::InputSavedStarGiftSlug(g) => g.slug.clone(),
        })
    }

    /// The second half of a Stars payment: the Bot API method that buys what the invoice names. The form id the
    /// first half made up isn't read.
    async fn pay(&self, body: &[u8], connection: Option<&str>) -> Result<Vec<u8>> {
        use base::InputInvoice as I;
        let r = f::payments::SendStarsForm::from_bytes(body)?;
        match (&r.invoice, connection) {
            (I::InputInvoiceStarGift(i), None) => {
                if i.hide_name {
                    return Err(unsupported("gifts that hide the sender's name"));
                }
                let mut out = Map::new();
                let chat_id = self.chat_id(&i.peer)?;
                let key = if chat_id > 0 { "user_id" } else { "chat_id" };
                out.insert(key.into(), chat_id.into());
                out.insert("gift_id".into(), i.gift_id.to_string().into());
                if i.include_upgrade {
                    out.insert("pay_for_upgrade".into(), true.into());
                }
                self.gift_text(&mut out, i.message.as_ref());
                self.call("sendGift", Value::Object(out)).await?;
            }
            (I::InputInvoicePremiumGiftStars(i), None) => {
                let star_count = PREMIUM_PRICES
                    .iter()
                    .find(|&&(months, _)| months == i.months)
                    .map(|&(_, stars)| stars)
                    .ok_or(Error::InvalidArgument("the Bot API gifts Premium only for 3, 6 or 12 months"))?;
                let mut out = Map::new();
                out.insert("user_id".into(), self.user_id(&i.user_id)?.into());
                out.insert("month_count".into(), i.months.into());
                out.insert("star_count".into(), star_count.into());
                self.gift_text(&mut out, i.message.as_ref());
                self.call("giftPremiumSubscription", Value::Object(out)).await?;
            }
            (I::InputInvoiceBusinessBotTransferStars(i), Some(connection)) => {
                let body = json!({ "business_connection_id": connection, "star_count": i.stars });
                self.call("transferBusinessAccountStars", body).await?;
            }
            (I::InputInvoiceStarGiftUpgrade(i), Some(connection)) => {
                self.upgrade(connection, &i.stargift, i.keep_original_details).await?;
            }
            (I::InputInvoiceStarGiftTransfer(i), Some(connection)) => {
                self.transfer(connection, &i.stargift, &i.to_id).await?;
            }
            _ => return Err(unsupported("paying this invoice in Stars")),
        }
        let updates = base::Updates::Updates(Box::new(types::Updates {
            updates: Vec::new(),
            users: Vec::new(),
            chats: Vec::new(),
            date: 0,
            seq: 0,
        }));
        Ok(types::payments::PaymentResult { updates }.to_bytes())
    }
}

/// The first half of a Stars payment: the Bot API pays in one call, so the form is made up, priced at nothing.
fn payment_form(body: &[u8]) -> Result<Vec<u8>> {
    f::payments::GetPaymentForm::from_bytes(body)?;
    let invoice = base::Invoice::Invoice(Box::new(types::Invoice::new("XTR".into(), Vec::new())));
    Ok(types::payments::PaymentFormStarGift { form_id: 0, invoice }.to_bytes())
}

/// A shop gift as TL's `starGift`; `None` when its sticker's file id doesn't decode.
fn star_gift(gift: &Gift, peers: &mut Peers) -> Option<base::StarGift> {
    let id = gift.id.parse().ok()?;
    let mut raw = types::StarGift::new(id, sticker_document(&gift.sticker)?, gift.star_count, 0);
    raw.upgrade_stars = gift.upgrade_star_count;
    raw.require_premium = gift.is_premium;
    raw.peer_color_available = gift.has_colors;
    raw.limited = gift.total_count.is_some();
    raw.availability_total = gift.total_count.map(int);
    raw.availability_remains = gift.remaining_count.map(int);
    raw.sold_out = gift.remaining_count == Some(0);
    raw.limited_per_user = gift.personal_total_count.is_some();
    raw.per_user_total = gift.personal_total_count.map(int);
    raw.per_user_remains = gift.personal_remaining_count.map(int);
    raw.upgrade_variants = gift.unique_gift_variant_count.map(int);
    raw.background = gift.background.as_ref().map(|c| {
        let colors = types::StarGiftBackground {
            center_color: int(c.center_color),
            edge_color: int(c.edge_color),
            text_color: int(c.text_color),
        };
        base::StarGiftBackground::StarGiftBackground(Box::new(colors))
    });
    raw.released_by = gift.publisher_chat.as_ref().map(|chat| {
        peers.chat(chat);
        get_peer(chat.id)
    });
    Some(base::StarGift::StarGift(Box::new(raw)))
}

/// A unique gift as TL's `starGiftUnique`. The Bot API gives no id of the unique gift itself, so it is 0.
fn unique_star_gift(gift: &UniqueGift, peers: &mut Peers) -> Result<base::StarGift> {
    let rarity = |per_mille: i64| {
        base::StarGiftAttributeRarity::StarGiftAttributeRarity(types::StarGiftAttributeRarity {
            permille: int(per_mille),
        })
    };
    let mut attributes = Vec::with_capacity(3);
    if let Some(model) = &gift.model {
        if let Some(document) = sticker_document(&model.sticker) {
            let model =
                types::StarGiftAttributeModel::new(model.name.clone(), document, rarity(model.rarity_per_mille));
            attributes.push(base::StarGiftAttribute::StarGiftAttributeModel(Box::new(model)));
        }
    }
    if let Some(symbol) = &gift.symbol {
        if let Some(document) = sticker_document(&symbol.sticker) {
            let symbol = types::StarGiftAttributePattern {
                name: symbol.name.clone(),
                document,
                rarity: rarity(symbol.rarity_per_mille),
            };
            attributes.push(base::StarGiftAttribute::StarGiftAttributePattern(Box::new(symbol)));
        }
    }
    if let Some(backdrop) = &gift.backdrop {
        let backdrop = types::StarGiftAttributeBackdrop {
            name: backdrop.name.clone(),
            backdrop_id: 0,
            center_color: int(backdrop.colors.center_color),
            edge_color: int(backdrop.colors.edge_color),
            pattern_color: int(backdrop.colors.symbol_color),
            text_color: int(backdrop.colors.text_color),
            rarity: rarity(backdrop.rarity_per_mille),
        };
        attributes.push(base::StarGiftAttribute::StarGiftAttributeBackdrop(Box::new(backdrop)));
    }
    let gift_id =
        gift.gift_id.parse().map_err(|_| Error::Unexpected("the Bot API sent a gift id that isn't a number"))?;
    let mut raw = types::StarGiftUnique::new(
        0,
        gift_id,
        gift.base_name.clone(),
        gift.name.clone(),
        int(gift.number),
        attributes,
        0,
        0,
    );
    raw.require_premium = gift.is_premium;
    raw.burned = gift.is_burned;
    raw.released_by = gift.publisher_chat.as_ref().map(|chat| {
        peers.chat(chat);
        get_peer(chat.id)
    });
    Ok(base::StarGift::StarGiftUnique(Box::new(raw)))
}

/// An owned gift as TL's `savedStarGift`; `None` for a kind added after this layer, or a regular gift whose
/// sticker doesn't decode. Its `owned_gift_id` comes back as a message id or a saved id when it has one of the
/// forms `owned_gift_id` in the friendly types has, else the gift has none.
fn saved_star_gift(owned: OwnedGift, peers: &mut Peers) -> Result<Option<types::SavedStarGift>> {
    let gift = match owned.kind.as_str() {
        "regular" => {
            let gift: Gift = parse(owned.gift)?;
            match star_gift(&gift, peers) {
                Some(gift) => gift,
                None => return Ok(None),
            }
        }
        "unique" => unique_star_gift(&parse::<UniqueGift>(owned.gift)?, peers)?,
        _ => return Ok(None),
    };
    let mut raw = types::SavedStarGift::new(int(owned.send_date), gift);
    raw.name_hidden = owned.is_private;
    raw.unsaved = owned.is_saved == Some(false);
    raw.refunded = owned.was_refunded;
    raw.can_upgrade = owned.can_be_upgraded;
    raw.upgrade_separate = owned.is_upgrade_separate;
    raw.convert_stars = owned.convert_star_count;
    raw.upgrade_stars = owned.prepaid_upgrade_star_count;
    raw.gift_num = owned.unique_gift_number.map(int);
    raw.transfer_stars = owned.transfer_star_count;
    raw.can_transfer_at = owned.next_transfer_date.map(int);
    if let Some(user) = &owned.sender_user {
        peers.user(user);
        raw.from_id = Some(get_peer(user.id));
    }
    if let Some(text) = owned.text {
        let entities = match owned.entities.as_deref() {
            Some(list) => convert::entities(list, peers),
            None => Vec::new(),
        };
        raw.message =
            Some(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities })));
    }
    if let Some((msg_id, saved_id)) = owned.owned_gift_id.as_deref().map(split_owned_gift_id) {
        raw.msg_id = msg_id;
        raw.saved_id = saved_id;
    }
    Ok(Some(raw))
}

/// A message id for `<message id>`, a saved id for `<chat id>_<saved id>`; neither for anything else.
fn split_owned_gift_id(id: &str) -> (Option<i32>, Option<i64>) {
    if let Ok(msg_id) = id.parse() {
        return (Some(msg_id), None);
    }
    match id.split_once('_').map(|(chat, saved)| (chat.parse::<i64>(), saved.parse::<i64>())) {
        Some((Ok(_), Ok(saved_id))) => (None, Some(saved_id)),
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::split_owned_gift_id;

    #[test]
    fn owned_gift_ids_split_as_the_friendly_types_write_them() {
        assert_eq!(split_owned_gift_id("42"), (Some(42), None));
        assert_eq!(split_owned_gift_id("-1000000000005_77"), (None, Some(77)));
        assert_eq!(split_owned_gift_id("PlushPepe-3"), (None, None));
    }
}
