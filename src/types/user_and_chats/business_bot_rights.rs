use crate::raw::{base, types};

/// What a bot may do in a business account it is connected to.
///
/// A field is `true` when the right is granted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BusinessBotRights {
    /// Send messages in the chats it handles.
    pub can_reply: bool,
    /// Mark the messages of the chats it handles as read.
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
    /// Grants nothing, like `Default`; grant rights with the setters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether the bot may send messages in the chats it handles.
    #[must_use]
    pub fn can_reply(mut self, can_reply: bool) -> Self {
        self.can_reply = can_reply;
        self
    }

    /// Sets whether the bot may mark the messages of the chats it handles as read.
    #[must_use]
    pub fn can_read_messages(mut self, can_read_messages: bool) -> Self {
        self.can_read_messages = can_read_messages;
        self
    }

    /// Sets whether the bot may delete messages the bot sent.
    #[must_use]
    pub fn can_delete_sent_messages(mut self, can_delete_sent_messages: bool) -> Self {
        self.can_delete_sent_messages = can_delete_sent_messages;
        self
    }

    /// Sets whether the bot may delete messages the account received.
    #[must_use]
    pub fn can_delete_all_messages(mut self, can_delete_all_messages: bool) -> Self {
        self.can_delete_all_messages = can_delete_all_messages;
        self
    }

    /// Sets whether the bot may change the account's name.
    #[must_use]
    pub fn can_edit_name(mut self, can_edit_name: bool) -> Self {
        self.can_edit_name = can_edit_name;
        self
    }

    /// Sets whether the bot may change the account's bio.
    #[must_use]
    pub fn can_edit_bio(mut self, can_edit_bio: bool) -> Self {
        self.can_edit_bio = can_edit_bio;
        self
    }

    /// Sets whether the bot may change the account's profile photo.
    #[must_use]
    pub fn can_edit_profile_photo(mut self, can_edit_profile_photo: bool) -> Self {
        self.can_edit_profile_photo = can_edit_profile_photo;
        self
    }

    /// Sets whether the bot may change the account's username.
    #[must_use]
    pub fn can_edit_username(mut self, can_edit_username: bool) -> Self {
        self.can_edit_username = can_edit_username;
        self
    }

    /// Sets whether the bot may change which gifts the account accepts.
    #[must_use]
    pub fn can_change_gift_settings(mut self, can_change_gift_settings: bool) -> Self {
        self.can_change_gift_settings = can_change_gift_settings;
        self
    }

    /// Sets whether the bot may view the account's gifts and Star balance.
    #[must_use]
    pub fn can_view_gifts_and_stars(mut self, can_view_gifts_and_stars: bool) -> Self {
        self.can_view_gifts_and_stars = can_view_gifts_and_stars;
        self
    }

    /// Sets whether the bot may convert the account's gifts to Stars.
    #[must_use]
    pub fn can_convert_gifts_to_stars(mut self, can_convert_gifts_to_stars: bool) -> Self {
        self.can_convert_gifts_to_stars = can_convert_gifts_to_stars;
        self
    }

    /// Sets whether the bot may transfer and upgrade the account's gifts.
    #[must_use]
    pub fn can_transfer_and_upgrade_gifts(mut self, can_transfer_and_upgrade_gifts: bool) -> Self {
        self.can_transfer_and_upgrade_gifts = can_transfer_and_upgrade_gifts;
        self
    }

    /// Sets whether the bot may transfer Stars from the account.
    #[must_use]
    pub fn can_transfer_stars(mut self, can_transfer_stars: bool) -> Self {
        self.can_transfer_stars = can_transfer_stars;
        self
    }

    /// Sets whether the bot may post and manage stories for the account.
    #[must_use]
    pub fn can_manage_stories(mut self, can_manage_stories: bool) -> Self {
        self.can_manage_stories = can_manage_stories;
        self
    }

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
    use crate::raw::base;

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

    #[test]
    fn builds_from_new_and_setters() {
        assert_eq!(BusinessBotRights::new(), BusinessBotRights::default());
        let rights = BusinessBotRights::new().can_reply(true).can_edit_bio(true).can_edit_bio(false);
        let base::BusinessBotRights::BusinessBotRights(raw) = rights.write();
        assert!(raw.reply && !raw.edit_bio && !raw.read_messages && !raw.delete_received_messages);
    }
}
