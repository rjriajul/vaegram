use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{ReactionType, UniqueGift};

/// The theme a chat's share card uses. A chat on the default theme has none.
/// Only [`Client::get_chat`](crate::Client::get_chat) fills it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShareTheme {
    /// Id of a [`ShareThemeBackdrop`](crate::types::ShareThemeBackdrop). `None` for a theme made from a gift
    /// or an emoji theme without a backdrop.
    pub backdrop_id: Option<i32>,
    /// Emoji drawn on the backdrop. Empty for a theme made from a gift.
    pub emojis: Vec<ReactionType>,
    /// The unique gift the theme comes from. `None` for an emoji backdrop theme or a gift that isn't unique.
    pub gift: Option<UniqueGift>,
}

impl ShareTheme {
    pub(crate) fn parse(raw: &base::ShareTheme, entities: &Entities) -> Option<ShareTheme> {
        match raw {
            base::ShareTheme::ShareThemeDefault => None,
            base::ShareTheme::ShareThemeEmojiBackdrop(t) => Some(ShareTheme {
                backdrop_id: t.backdrop_id,
                emojis: t.emojis.iter().flatten().filter_map(ReactionType::parse).collect(),
                gift: None,
            }),
            base::ShareTheme::ShareThemeGift(t) => Some(ShareTheme {
                backdrop_id: None,
                emojis: Vec::new(),
                gift: match &t.stargift {
                    base::StarGift::StarGiftUnique(g) => Some(UniqueGift::parse(g, entities)),
                    base::StarGift::StarGift(_) => None,
                },
            }),
            // Only sent to Telegram, never received.
            base::ShareTheme::InputShareThemeGift(_) => None,
        }
    }
}
