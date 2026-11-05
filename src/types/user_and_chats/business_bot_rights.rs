use crate::raw::{base, types};

/// What a bot may do in a business account it is connected to. A field is `true` when the right is granted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BusinessBotRights {
    /// Send messages in the chats it handles.
    pub can_reply: bool,
    /// Read the messages of the chats it handles.
    pub can_read_messages: bool,
    /// Delete messages the bot sent.
    pub can_delete_sent_messages: bool,
    /// Delete messages the account received.
    pub can_delete_all_messages: bool,
    /// Change the account's name.
    pub can_edit_name: bool,
    /// Change the account's bio.
    pub can_edit_bio: bool,
    /// Change the account's profile photo.
    pub can_edit_profile_photo: bool,
    /// Change the account's username.
    pub can_edit_username: bool,
    /// Change which gifts the account accepts.
    pub can_change_gift_settings: bool,
    /// View the account's gifts and Star balance.
    pub can_view_gifts_and_stars: bool,
    /// Convert the account's gifts to Stars.
    pub can_convert_gifts_to_stars: bool,
    /// Transfer and upgrade the account's gifts.
    pub can_transfer_and_upgrade_gifts: bool,
    /// Transfer Stars from the account.
    pub can_transfer_stars: bool,
    /// Post and manage stories for the account.
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

    pub(crate) fn write(&self) -> base::BusinessBotRights {
        let rights = types::BusinessBotRights {
            reply: self.can_reply,
            read_messages: self.can_read_messages,
            delete_sent_messages: self.can_delete_sent_messages,
            delete_received_messages: self.can_delete_all_messages,
            edit_name: self.can_edit_name,
            edit_bio: self.can_edit_bio,
            edit_profile_photo: self.can_edit_profile_photo,
            edit_username: self.can_edit_username,
            view_gifts: self.can_view_gifts_and_stars,
            sell_gifts: self.can_convert_gifts_to_stars,
            change_gift_settings: self.can_change_gift_settings,
            transfer_and_upgrade_gifts: self.can_transfer_and_upgrade_gifts,
            transfer_stars: self.can_transfer_stars,
            manage_stories: self.can_manage_stories,
        };
        base::BusinessBotRights::BusinessBotRights(Box::new(rights))
    }
}

#[cfg(test)]
mod tests {
    use super::BusinessBotRights;

    #[test]
    fn every_right_survives_a_write_and_parse() {
        let each: [fn(&mut BusinessBotRights); 14] = [
            |r| r.can_reply = true,
            |r| r.can_read_messages = true,
            |r| r.can_delete_sent_messages = true,
            |r| r.can_delete_all_messages = true,
            |r| r.can_edit_name = true,
            |r| r.can_edit_bio = true,
            |r| r.can_edit_profile_photo = true,
            |r| r.can_edit_username = true,
            |r| r.can_change_gift_settings = true,
            |r| r.can_view_gifts_and_stars = true,
            |r| r.can_convert_gifts_to_stars = true,
            |r| r.can_transfer_and_upgrade_gifts = true,
            |r| r.can_transfer_stars = true,
            |r| r.can_manage_stories = true,
        ];
        for set in each {
            let mut rights = BusinessBotRights::default();
            set(&mut rights);
            assert_eq!(BusinessBotRights::parse(&rights.write()), rights);
        }
    }
}
