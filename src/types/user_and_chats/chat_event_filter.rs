use crate::raw::{base, types};

/// Selects which kinds of events an admin log query returns.
///
/// With every field `false`, the query isn't filtered and returns all events.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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
}
