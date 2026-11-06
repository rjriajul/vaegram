use crate::raw::{base, types};

/// What an administrator may do. `Default` grants nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ChatPrivileges {
    /// Whether the admin's messages show the chat as sender instead of their name.
    pub is_anonymous: bool,
    /// Whether the admin can access the admin log, statistics and other chat management tools.
    pub can_manage_chat: bool,
    /// Whether the admin can delete other members' messages.
    pub can_delete_messages: bool,
    /// Whether the admin can start and manage video chats.
    pub can_manage_video_chats: bool,
    /// Whether the admin can ban and restrict members.
    pub can_restrict_members: bool,
    /// Whether the admin can add other administrators.
    pub can_promote_members: bool,
    /// Whether the admin can change the chat title, photo and other settings.
    pub can_change_info: bool,
    /// Whether the admin can add members and manage invite links.
    pub can_invite_users: bool,
    /// Whether the admin can post stories on behalf of the chat.
    pub can_post_stories: bool,
    /// Whether the admin can edit stories the chat's other admins posted.
    pub can_edit_stories: bool,
    /// Whether the admin can delete stories the chat's other admins posted.
    pub can_delete_stories: bool,
    /// Whether the admin can post messages in a channel.
    pub can_post_messages: bool,
    /// Whether the admin can edit other members' messages in a channel.
    pub can_edit_messages: bool,
    /// Whether the admin can pin messages.
    pub can_pin_messages: bool,
    /// Whether the admin can create, edit and close forum topics.
    pub can_manage_topics: bool,
    /// Whether the admin can manage the channel's direct messages.
    pub can_manage_direct_messages: bool,
    /// Whether the admin can edit members' tags.
    pub can_manage_tags: bool,
    /// Whether the admin can send welcome messages.
    pub can_send_welcome_messages: bool,
}

impl ChatPrivileges {
    /// Grants nothing, like `Default`; grant privileges with the setters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether the admin's messages show the chat as sender instead of their name.
    #[must_use]
    pub fn is_anonymous(mut self, is_anonymous: bool) -> Self {
        self.is_anonymous = is_anonymous;
        self
    }

    /// Sets whether the admin can access the admin log, statistics and other chat management tools.
    #[must_use]
    pub fn can_manage_chat(mut self, can_manage_chat: bool) -> Self {
        self.can_manage_chat = can_manage_chat;
        self
    }

    /// Sets whether the admin can delete other members' messages.
    #[must_use]
    pub fn can_delete_messages(mut self, can_delete_messages: bool) -> Self {
        self.can_delete_messages = can_delete_messages;
        self
    }

    /// Sets whether the admin can start and manage video chats.
    #[must_use]
    pub fn can_manage_video_chats(mut self, can_manage_video_chats: bool) -> Self {
        self.can_manage_video_chats = can_manage_video_chats;
        self
    }

    /// Sets whether the admin can ban and restrict members.
    #[must_use]
    pub fn can_restrict_members(mut self, can_restrict_members: bool) -> Self {
        self.can_restrict_members = can_restrict_members;
        self
    }

    /// Sets whether the admin can add other administrators.
    #[must_use]
    pub fn can_promote_members(mut self, can_promote_members: bool) -> Self {
        self.can_promote_members = can_promote_members;
        self
    }

    /// Sets whether the admin can change the chat title, photo and other settings.
    #[must_use]
    pub fn can_change_info(mut self, can_change_info: bool) -> Self {
        self.can_change_info = can_change_info;
        self
    }

    /// Sets whether the admin can add members and manage invite links.
    #[must_use]
    pub fn can_invite_users(mut self, can_invite_users: bool) -> Self {
        self.can_invite_users = can_invite_users;
        self
    }

    /// Sets whether the admin can post stories on behalf of the chat.
    #[must_use]
    pub fn can_post_stories(mut self, can_post_stories: bool) -> Self {
        self.can_post_stories = can_post_stories;
        self
    }

    /// Sets whether the admin can edit stories the chat's other admins posted.
    #[must_use]
    pub fn can_edit_stories(mut self, can_edit_stories: bool) -> Self {
        self.can_edit_stories = can_edit_stories;
        self
    }

    /// Sets whether the admin can delete stories the chat's other admins posted.
    #[must_use]
    pub fn can_delete_stories(mut self, can_delete_stories: bool) -> Self {
        self.can_delete_stories = can_delete_stories;
        self
    }

    /// Sets whether the admin can post messages in a channel.
    #[must_use]
    pub fn can_post_messages(mut self, can_post_messages: bool) -> Self {
        self.can_post_messages = can_post_messages;
        self
    }

    /// Sets whether the admin can edit other members' messages in a channel.
    #[must_use]
    pub fn can_edit_messages(mut self, can_edit_messages: bool) -> Self {
        self.can_edit_messages = can_edit_messages;
        self
    }

    /// Sets whether the admin can pin messages.
    #[must_use]
    pub fn can_pin_messages(mut self, can_pin_messages: bool) -> Self {
        self.can_pin_messages = can_pin_messages;
        self
    }

    /// Sets whether the admin can create, edit and close forum topics.
    #[must_use]
    pub fn can_manage_topics(mut self, can_manage_topics: bool) -> Self {
        self.can_manage_topics = can_manage_topics;
        self
    }

    /// Sets whether the admin can manage the channel's direct messages.
    #[must_use]
    pub fn can_manage_direct_messages(mut self, can_manage_direct_messages: bool) -> Self {
        self.can_manage_direct_messages = can_manage_direct_messages;
        self
    }

    /// Sets whether the admin can edit members' tags.
    #[must_use]
    pub fn can_manage_tags(mut self, can_manage_tags: bool) -> Self {
        self.can_manage_tags = can_manage_tags;
        self
    }

    /// Sets whether the admin can send welcome messages.
    #[must_use]
    pub fn can_send_welcome_messages(mut self, can_send_welcome_messages: bool) -> Self {
        self.can_send_welcome_messages = can_send_welcome_messages;
        self
    }

    pub(crate) fn parse(rights: &base::ChatAdminRights) -> ChatPrivileges {
        let base::ChatAdminRights::ChatAdminRights(r) = rights;
        ChatPrivileges {
            is_anonymous: r.anonymous,
            can_manage_chat: r.other,
            can_delete_messages: r.delete_messages,
            can_manage_video_chats: r.manage_call,
            can_restrict_members: r.ban_users,
            can_promote_members: r.add_admins,
            can_change_info: r.change_info,
            can_invite_users: r.invite_users,
            can_post_stories: r.post_stories,
            can_edit_stories: r.edit_stories,
            can_delete_stories: r.delete_stories,
            can_post_messages: r.post_messages,
            can_edit_messages: r.edit_messages,
            can_pin_messages: r.pin_messages,
            can_manage_topics: r.manage_topics,
            can_manage_direct_messages: r.manage_direct_messages,
            can_manage_tags: r.manage_ranks,
            can_send_welcome_messages: r.manage_welcome_messages,
        }
    }

    pub(crate) fn write(&self) -> base::ChatAdminRights {
        base::ChatAdminRights::ChatAdminRights(Box::new(types::ChatAdminRights {
            change_info: self.can_change_info,
            post_messages: self.can_post_messages,
            edit_messages: self.can_edit_messages,
            delete_messages: self.can_delete_messages,
            ban_users: self.can_restrict_members,
            invite_users: self.can_invite_users,
            pin_messages: self.can_pin_messages,
            add_admins: self.can_promote_members,
            anonymous: self.is_anonymous,
            manage_call: self.can_manage_video_chats,
            other: self.can_manage_chat,
            manage_topics: self.can_manage_topics,
            post_stories: self.can_post_stories,
            edit_stories: self.can_edit_stories,
            delete_stories: self.can_delete_stories,
            manage_direct_messages: self.can_manage_direct_messages,
            manage_ranks: self.can_manage_tags,
            manage_linked_peers: false,
            manage_welcome_messages: self.can_send_welcome_messages,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::ChatPrivileges;
    use crate::raw::Serializable;

    fn flags(privileges: ChatPrivileges) -> u32 {
        u32::from_le_bytes(privileges.write().to_bytes()[4..8].try_into().unwrap())
    }

    #[test]
    fn maps_to_admin_rights_flags() {
        assert_eq!(flags(ChatPrivileges::default()), 0);
        assert_eq!(flags(ChatPrivileges { can_send_welcome_messages: true, ..Default::default() }), 1 << 20);
        assert_eq!(flags(ChatPrivileges { can_manage_chat: true, ..Default::default() }), 1 << 12);
        assert_eq!(flags(ChatPrivileges { can_manage_tags: true, ..Default::default() }), 1 << 18);
        assert_eq!(flags(ChatPrivileges { can_restrict_members: true, ..Default::default() }), 1 << 4);
        assert_eq!(flags(ChatPrivileges { can_manage_video_chats: true, ..Default::default() }), 1 << 11);
    }

    #[test]
    fn parses_back() {
        let privileges = ChatPrivileges {
            is_anonymous: true,
            can_promote_members: true,
            can_send_welcome_messages: true,
            can_manage_direct_messages: true,
            ..Default::default()
        };
        assert_eq!(ChatPrivileges::parse(&privileges.write()), privileges);
    }

    #[test]
    fn builds_from_new_and_setters() {
        assert_eq!(flags(ChatPrivileges::new()), 0);
        let pinner = ChatPrivileges::new().can_pin_messages(true).can_manage_tags(true);
        assert_eq!(flags(pinner.can_manage_tags(false)), 1 << 7);
    }
}
