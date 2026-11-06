use crate::raw::base;

/// The emoji shown next to a user's name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EmojiStatus {
    /// Id of the custom emoji.
    pub custom_emoji_id: i64,
    /// Unix time the status expires. `None` when it doesn't.
    pub until: Option<i32>,
    /// Id of the unique gift when the status is one. `None` for a plain emoji status, as are all fields below.
    pub collectible_id: Option<i64>,
    /// Slug of the unique gift, as in `t.me/nft/<name>`.
    pub name: Option<String>,
    /// Title of the unique gift.
    pub title: Option<String>,
    /// Id of the custom emoji that makes up the backdrop pattern.
    pub pattern_custom_emoji_id: Option<i64>,
    /// Color at the center of the backdrop.
    pub center_color: Option<i32>,
    /// Color at the edge of the backdrop.
    pub edge_color: Option<i32>,
    /// Color of the backdrop pattern.
    pub pattern_color: Option<i32>,
    /// Color of the text on the backdrop.
    pub text_color: Option<i32>,
}

impl EmojiStatus {
    pub(crate) fn parse(status: &base::EmojiStatus) -> Option<EmojiStatus> {
        match status {
            base::EmojiStatus::EmojiStatus(s) => Some(EmojiStatus {
                custom_emoji_id: s.document_id,
                until: s.until,
                collectible_id: None,
                name: None,
                title: None,
                pattern_custom_emoji_id: None,
                center_color: None,
                edge_color: None,
                pattern_color: None,
                text_color: None,
            }),
            base::EmojiStatus::EmojiStatusCollectible(s) => Some(EmojiStatus {
                custom_emoji_id: s.document_id,
                until: s.until,
                collectible_id: Some(s.collectible_id),
                name: Some(s.slug.clone()),
                title: Some(s.title.clone()),
                pattern_custom_emoji_id: Some(s.pattern_document_id),
                center_color: Some(s.center_color),
                edge_color: Some(s.edge_color),
                pattern_color: Some(s.pattern_color),
                text_color: Some(s.text_color),
            }),
            _ => None,
        }
    }

    // Requests go out with hash 0, so the not-modified answer never comes back.
    pub(crate) fn parse_list(statuses: base::account::EmojiStatuses) -> Vec<EmojiStatus> {
        match statuses {
            base::account::EmojiStatuses::EmojiStatuses(s) => s.statuses.iter().filter_map(Self::parse).collect(),
            base::account::EmojiStatuses::EmojiStatusesNotModified => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EmojiStatus;
    use crate::raw::{base, types};

    #[test]
    fn reads_plain_and_collectible_statuses() {
        let plain = base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus { document_id: 5, until: Some(9) }));
        let gift = base::EmojiStatus::EmojiStatusCollectible(Box::new(types::EmojiStatusCollectible {
            collectible_id: 7,
            document_id: 6,
            title: "Neko Helmet".into(),
            slug: "NekoHelmet-9215".into(),
            pattern_document_id: 1,
            center_color: 2,
            edge_color: 3,
            pattern_color: 4,
            text_color: 0,
            until: None,
        }));
        let list = base::account::EmojiStatuses::EmojiStatuses(Box::new(types::account::EmojiStatuses {
            hash: 0,
            statuses: vec![plain, base::EmojiStatus::EmojiStatusEmpty, gift],
        }));
        let parsed = EmojiStatus::parse_list(list);
        assert_eq!(parsed.len(), 2);
        let (plain, gift) = (&parsed[0], &parsed[1]);
        assert_eq!(
            (plain.custom_emoji_id, plain.until, plain.collectible_id, plain.name.as_deref()),
            (5, Some(9), None, None)
        );
        let plain_gift_fields = (&plain.title, plain.pattern_custom_emoji_id, plain.center_color, plain.edge_color);
        assert_eq!(plain_gift_fields, (&None, None, None, None));
        assert_eq!((plain.pattern_color, plain.text_color), (None, None));
        assert_eq!((gift.custom_emoji_id, gift.until, gift.collectible_id), (6, None, Some(7)));
        assert_eq!((gift.name.as_deref(), gift.title.as_deref()), (Some("NekoHelmet-9215"), Some("Neko Helmet")));
        let colors =
            (gift.pattern_custom_emoji_id, gift.center_color, gift.edge_color, gift.pattern_color, gift.text_color);
        assert_eq!(colors, (Some(1), Some(2), Some(3), Some(4), Some(0)));
    }
}
