use crate::raw::{base, types};

/// Selects which kinds of events an admin log query returns.
///
/// With every field `false`, the query isn't filtered and returns all events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ChatEventFilter {
    /// Bans, unbans, kicks and restriction changes.
    pub new_restrictions: bool,
    /// Promotions and demotions of administrators.
    pub new_privileges: bool,
    /// Users joining and being invited.
    pub new_members: bool,
    /// Changes to the title, description, username and photo.
    pub chat_info: bool,
    /// Changes to chat settings.
    pub chat_settings: bool,
    /// Invite links being created, edited, revoked or deleted.
    pub invite_links: bool,
    /// Deleted messages.
    pub deleted_messages: bool,
    /// Edited messages.
    pub edited_messages: bool,
    /// Pinned and unpinned messages.
    pub pinned_messages: bool,
    /// Users leaving.
    pub leaving_members: bool,
    /// Video chats starting and ending.
    pub video_chats: bool,
    /// Messages sent by admins.
    pub sent_messages: bool,
    /// Changes to forum topics.
    pub forum_topics: bool,
    /// Members extending their paid subscription.
    pub subscription_extensions: bool,
    /// Changes to member ranks.
    pub member_ranks: bool,
}

impl ChatEventFilter {
    /// Filters nothing, so the query returns every event; each setter narrows it to the kinds turned on.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether the query returns bans, unbans, kicks and restriction changes.
    #[must_use]
    pub fn new_restrictions(mut self, new_restrictions: bool) -> Self {
        self.new_restrictions = new_restrictions;
        self
    }

    /// Sets whether the query returns promotions and demotions of administrators.
    #[must_use]
    pub fn new_privileges(mut self, new_privileges: bool) -> Self {
        self.new_privileges = new_privileges;
        self
    }

    /// Sets whether the query returns users joining and being invited.
    #[must_use]
    pub fn new_members(mut self, new_members: bool) -> Self {
        self.new_members = new_members;
        self
    }

    /// Sets whether the query returns changes to the title, description, username and photo.
    #[must_use]
    pub fn chat_info(mut self, chat_info: bool) -> Self {
        self.chat_info = chat_info;
        self
    }

    /// Sets whether the query returns changes to chat settings.
    #[must_use]
    pub fn chat_settings(mut self, chat_settings: bool) -> Self {
        self.chat_settings = chat_settings;
        self
    }

    /// Sets whether the query returns invite links being created, edited, revoked or deleted.
    #[must_use]
    pub fn invite_links(mut self, invite_links: bool) -> Self {
        self.invite_links = invite_links;
        self
    }

    /// Sets whether the query returns deleted messages.
    #[must_use]
    pub fn deleted_messages(mut self, deleted_messages: bool) -> Self {
        self.deleted_messages = deleted_messages;
        self
    }

    /// Sets whether the query returns edited messages.
    #[must_use]
    pub fn edited_messages(mut self, edited_messages: bool) -> Self {
        self.edited_messages = edited_messages;
        self
    }

    /// Sets whether the query returns pinned and unpinned messages.
    #[must_use]
    pub fn pinned_messages(mut self, pinned_messages: bool) -> Self {
        self.pinned_messages = pinned_messages;
        self
    }

    /// Sets whether the query returns users leaving.
    #[must_use]
    pub fn leaving_members(mut self, leaving_members: bool) -> Self {
        self.leaving_members = leaving_members;
        self
    }

    /// Sets whether the query returns video chats starting and ending.
    #[must_use]
    pub fn video_chats(mut self, video_chats: bool) -> Self {
        self.video_chats = video_chats;
        self
    }

    /// Sets whether the query returns messages sent by admins.
    #[must_use]
    pub fn sent_messages(mut self, sent_messages: bool) -> Self {
        self.sent_messages = sent_messages;
        self
    }

    /// Sets whether the query returns changes to forum topics.
    #[must_use]
    pub fn forum_topics(mut self, forum_topics: bool) -> Self {
        self.forum_topics = forum_topics;
        self
    }

    /// Sets whether the query returns members extending their paid subscription.
    #[must_use]
    pub fn subscription_extensions(mut self, subscription_extensions: bool) -> Self {
        self.subscription_extensions = subscription_extensions;
        self
    }

    /// Sets whether the query returns changes to member ranks.
    #[must_use]
    pub fn member_ranks(mut self, member_ranks: bool) -> Self {
        self.member_ranks = member_ranks;
        self
    }

    pub(crate) fn write(&self) -> Option<base::ChannelAdminLogEventsFilter> {
        if *self == ChatEventFilter::default() {
            return None;
        }
        let filter = types::ChannelAdminLogEventsFilter {
            join: self.new_members,
            leave: self.leaving_members,
            invite: self.new_members,
            ban: self.new_restrictions,
            unban: self.new_restrictions,
            kick: self.new_restrictions,
            unkick: self.new_restrictions,
            promote: self.new_privileges,
            demote: self.new_privileges,
            info: self.chat_info,
            settings: self.chat_settings,
            pinned: self.pinned_messages,
            edit: self.edited_messages,
            delete: self.deleted_messages,
            group_call: self.video_chats,
            invites: self.invite_links,
            send: self.sent_messages,
            forums: self.forum_topics,
            sub_extend: self.subscription_extensions,
            edit_rank: self.member_ranks,
        };
        Some(base::ChannelAdminLogEventsFilter::ChannelAdminLogEventsFilter(Box::new(filter)))
    }
}

#[cfg(test)]
mod tests {
    use super::ChatEventFilter;
    use crate::raw::base;

    #[test]
    fn an_empty_filter_asks_for_everything() {
        assert_eq!(ChatEventFilter::default().write(), None);
        let filter = ChatEventFilter { new_restrictions: true, leaving_members: true, ..Default::default() };
        let Some(base::ChannelAdminLogEventsFilter::ChannelAdminLogEventsFilter(f)) = filter.write() else {
            panic!("expected a filter");
        };
        assert_eq!((f.ban, f.unban, f.kick, f.unkick, f.leave), (true, true, true, true, true));
        assert_eq!((f.join, f.promote, f.edit, f.delete), (false, false, false, false));
    }

    #[test]
    fn builds_from_new_and_setters() {
        assert_eq!(ChatEventFilter::new().write(), None);
        let filter = ChatEventFilter::new().new_members(true).edited_messages(true).edited_messages(false);
        let Some(base::ChannelAdminLogEventsFilter::ChannelAdminLogEventsFilter(f)) = filter.write() else {
            panic!("expected a filter");
        };
        assert_eq!((f.join, f.invite, f.edit, f.leave), (true, true, false, false));
    }
}
