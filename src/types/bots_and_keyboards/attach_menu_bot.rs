use crate::dispatcher::Entities;
use crate::enums::AttachMenuPeerType;
use crate::raw::types;
use crate::types::User;

#[derive(Debug, Clone, PartialEq)]
pub struct AttachMenuBot {
    pub bot: User,
    pub short_name: String,
    pub inactive: bool,
    pub has_settings: bool,
    pub request_write_access: bool,
    pub show_in_attach_menu: bool,
    pub show_in_side_menu: bool,
    pub side_menu_disclaimer_needed: bool,
    pub peer_types: Vec<AttachMenuPeerType>,
}

impl AttachMenuBot {
    pub(crate) fn parse(raw: types::AttachMenuBot, entities: &Entities) -> AttachMenuBot {
        AttachMenuBot {
            bot: User::parse(raw.bot_id, entities),
            short_name: raw.short_name,
            inactive: raw.inactive,
            has_settings: raw.has_settings,
            request_write_access: raw.request_write_access,
            show_in_attach_menu: raw.show_in_attach_menu,
            show_in_side_menu: raw.show_in_side_menu,
            side_menu_disclaimer_needed: raw.side_menu_disclaimer_needed,
            peer_types: raw.peer_types.iter().flatten().map(AttachMenuPeerType::parse).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AttachMenuBot;
    use crate::dispatcher::Entities;
    use crate::enums::AttachMenuPeerType as P;
    use crate::raw::{base, types};

    #[test]
    fn keeps_flags_and_maps_peer_types() {
        let raw = |peer_types| types::AttachMenuBot {
            inactive: true,
            has_settings: false,
            request_write_access: true,
            show_in_attach_menu: true,
            show_in_side_menu: false,
            side_menu_disclaimer_needed: true,
            bot_id: 42,
            short_name: "shop".into(),
            peer_types,
            icons: Vec::new(),
        };
        let peer_types = vec![
            base::AttachMenuPeerType::AttachMenuPeerTypeSameBotPM,
            base::AttachMenuPeerType::AttachMenuPeerTypeChat,
            base::AttachMenuPeerType::AttachMenuPeerTypeBroadcast,
        ];
        let bot = AttachMenuBot::parse(raw(Some(peer_types)), &Entities::default());
        assert_eq!((bot.bot.id, bot.short_name.as_str()), (42, "shop"));
        assert_eq!(
            (bot.inactive, bot.has_settings, bot.request_write_access, bot.show_in_attach_menu),
            (true, false, true, true)
        );
        assert_eq!((bot.show_in_side_menu, bot.side_menu_disclaimer_needed), (false, true));
        assert_eq!(bot.peer_types, [P::SameBot, P::Group, P::Channel]);

        assert!(AttachMenuBot::parse(raw(None), &Entities::default()).peer_types.is_empty());
    }
}
