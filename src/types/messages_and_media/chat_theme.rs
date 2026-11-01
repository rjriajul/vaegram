use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::UniqueGift;

/// A chat theme: a named one, or one taken from a unique gift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTheme {
    /// The emoji that names the theme.
    pub name: Option<String>,
    pub gift: Option<UniqueGift>,
}

impl ChatTheme {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn parse(raw: &base::ChatTheme, entities: &Entities) -> ChatTheme {
        match raw {
            base::ChatTheme::ChatTheme(t) => ChatTheme { name: Some(t.emoticon.clone()), gift: None },
            base::ChatTheme::ChatThemeUniqueGift(t) => {
                let gift = match &t.gift {
                    base::StarGift::StarGiftUnique(g) => Some(UniqueGift::parse(g, entities)),
                    base::StarGift::StarGift(_) => None,
                };
                ChatTheme { name: None, gift }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatTheme;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn a_theme_is_named_or_comes_from_a_gift() {
        let entities = Entities::default();
        let named = base::ChatTheme::ChatTheme(types::ChatTheme { emoticon: "🐱".into() }.into());
        assert_eq!(ChatTheme::parse(&named, &entities), ChatTheme { name: Some("🐱".into()), gift: None });
        let gift = types::StarGiftUnique {
            require_premium: false,
            resale_ton_only: false,
            theme_available: true,
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
        };
        let raw = types::ChatThemeUniqueGift {
            gift: base::StarGift::StarGiftUnique(gift.into()),
            theme_settings: Vec::new(),
        };
        let theme = ChatTheme::parse(&base::ChatTheme::ChatThemeUniqueGift(raw.into()), &entities);
        let gift = theme.gift.unwrap();
        assert_eq!((theme.name, gift.gift_id, gift.name.as_str()), (None, 2, "PlushPepe-3"));
    }
}
