use crate::raw::base;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BusinessBotRights {
    pub can_reply: bool,
    pub can_read_messages: bool,
    pub can_delete_sent_messages: bool,
    pub can_delete_all_messages: bool,
    pub can_edit_name: bool,
    pub can_edit_bio: bool,
    pub can_edit_profile_photo: bool,
    pub can_edit_username: bool,
    pub can_change_gift_settings: bool,
    pub can_view_gifts_and_stars: bool,
    pub can_convert_gifts_to_stars: bool,
    pub can_transfer_and_upgrade_gifts: bool,
    pub can_transfer_stars: bool,
    pub can_manage_stories: bool,
}

impl BusinessBotRights {
    pub(crate) fn parse(raw: &base::BusinessBotRights) -> BusinessBotRights {
        let base::BusinessBotRights::BusinessBotRights(r) = raw;
        BusinessBotRights {
            can_reply: r.reply,
            can_read_messages: r.read_messages,
            can_delete_sent_messages: r.delete_sent_messages,
            can_delete_all_messages: r.delete_received_messages,
            can_edit_name: r.edit_name,
            can_edit_bio: r.edit_bio,
            can_edit_profile_photo: r.edit_profile_photo,
            can_edit_username: r.edit_username,
            can_change_gift_settings: r.change_gift_settings,
            can_view_gifts_and_stars: r.view_gifts,
            can_convert_gifts_to_stars: r.sell_gifts,
            can_transfer_and_upgrade_gifts: r.transfer_and_upgrade_gifts,
            can_transfer_stars: r.transfer_stars,
            can_manage_stories: r.manage_stories,
        }
    }
}
