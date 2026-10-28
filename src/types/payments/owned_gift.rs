use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::messages_and_media::text_with_entities;
use crate::types::{Gift, MessageEntity, UniqueGift, User};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedGift {
    Regular(OwnedGiftRegular),
    Unique(OwnedGiftUnique),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGiftRegular {
    pub gift: Gift,
    pub owned_gift_id: Option<String>,
    pub sender_user: Option<User>,
    pub send_date: i32,
    pub text: Option<String>,
    pub entities: Option<Vec<MessageEntity>>,
    pub is_private: bool,
    pub is_saved: bool,
    pub can_be_upgraded: bool,
    pub was_refunded: bool,
    pub convert_star_count: Option<i64>,
    pub prepaid_upgrade_star_count: Option<i64>,
    pub is_upgrade_separate: bool,
    pub unique_gift_number: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGiftUnique {
    pub gift: UniqueGift,
    pub owned_gift_id: Option<String>,
    pub sender_user: Option<User>,
    pub send_date: i32,
    pub is_saved: bool,
    pub can_be_transferred: bool,
    pub transfer_star_count: Option<i64>,
    pub next_transfer_date: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGifts {
    pub total_count: i32,
    pub gifts: Vec<OwnedGift>,
    pub next_offset: Option<String>,
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
                OwnedGift::Regular(OwnedGiftRegular {
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
                })
            }
            base::StarGift::StarGiftUnique(g) => OwnedGift::Unique(OwnedGiftUnique {
                gift: UniqueGift::parse(g, entities),
                owned_gift_id,
                sender_user,
                send_date: raw.date,
                is_saved: !raw.unsaved,
                can_be_transferred: raw.transfer_stars.is_some() || raw.can_transfer_at.is_some(),
                transfer_star_count: raw.transfer_stars,
                next_transfer_date: raw.can_transfer_at,
            }),
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
                base::InputSavedStarGift::InputSavedStarGiftUser(Box::new(types::InputSavedStarGiftUser { msg_id }))
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
}

#[cfg(test)]
mod tests {
    use super::{OwnedGiftRef, owned_gift_id, parse_owned_gift_id};

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
}
