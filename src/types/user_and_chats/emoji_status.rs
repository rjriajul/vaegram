use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmojiStatus {
    pub custom_emoji_id: i64,
    pub until: Option<i32>,
    /// Set when the status is a unique gift; `name` is its slug from `t.me/nft/<name>`.
    pub collectible_id: Option<i64>,
    pub name: Option<String>,
}

impl EmojiStatus {
    pub(crate) fn parse(status: &base::EmojiStatus) -> Option<EmojiStatus> {
        match status {
            base::EmojiStatus::EmojiStatus(s) => {
                Some(EmojiStatus { custom_emoji_id: s.document_id, until: s.until, collectible_id: None, name: None })
            }
            base::EmojiStatus::EmojiStatusCollectible(s) => Some(EmojiStatus {
                custom_emoji_id: s.document_id,
                until: s.until,
                collectible_id: Some(s.collectible_id),
                name: Some(s.slug.clone()),
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
            center_color: 0,
            edge_color: 0,
            pattern_color: 0,
            text_color: 0,
            until: None,
        }));
        let list = base::account::EmojiStatuses::EmojiStatuses(Box::new(types::account::EmojiStatuses {
            hash: 0,
            statuses: vec![plain, base::EmojiStatus::EmojiStatusEmpty, gift],
        }));
        let plain = EmojiStatus { custom_emoji_id: 5, until: Some(9), collectible_id: None, name: None };
        let gift = EmojiStatus {
            custom_emoji_id: 6,
            until: None,
            collectible_id: Some(7),
            name: Some("NekoHelmet-9215".into()),
        };
        assert_eq!(EmojiStatus::parse_list(list), [plain, gift]);
    }
}
