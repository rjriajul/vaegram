use crate::raw::base;

/// The reply or profile color of a user or chat, as ids into the lists from `get_peer_colors` and
/// `get_peer_profile_colors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatColor {
    pub color: Option<i32>,
    pub background_emoji_id: Option<i64>,
}

impl ChatColor {
    /// `None` for collectible colors, which come from a unique gift instead of the lists.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn parse(raw: &base::PeerColor) -> Option<ChatColor> {
        match raw {
            base::PeerColor::PeerColor(c) => {
                Some(ChatColor { color: c.color, background_emoji_id: c.background_emoji_id })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatColor;
    use crate::raw::{base, types};

    #[test]
    fn only_plain_peer_colors_parse() {
        let plain =
            base::PeerColor::PeerColor(types::PeerColor { color: Some(5), background_emoji_id: Some(9) }.into());
        assert_eq!(ChatColor::parse(&plain), Some(ChatColor { color: Some(5), background_emoji_id: Some(9) }));
        let input = base::PeerColor::InputPeerColorCollectible(types::InputPeerColorCollectible { collectible_id: 1 });
        assert_eq!(ChatColor::parse(&input), None);
    }
}
