use crate::raw::{base, types};

/// What a member without admin rights may do. `Default` allows nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ChatPermissions {
    /// Whether members can send plain text messages.
    pub can_send_messages: bool,
    /// Whether members can send music files.
    pub can_send_audios: bool,
    /// Whether members can send general files.
    pub can_send_documents: bool,
    /// Whether members can send photos.
    pub can_send_photos: bool,
    /// Whether members can send videos.
    pub can_send_videos: bool,
    /// Whether members can send round video messages.
    pub can_send_video_notes: bool,
    /// Whether members can send voice messages.
    pub can_send_voice_notes: bool,
    /// Whether members can send polls.
    pub can_send_polls: bool,
    /// Whether members can send stickers, GIFs and games, and use inline bots.
    pub can_send_other_messages: bool,
    /// Whether members can embed link previews in messages.
    pub can_add_web_page_previews: bool,
    /// Whether members can react to messages.
    pub can_react_to_messages: bool,
    /// Whether members can edit their own member tag (their rank).
    pub can_edit_tag: bool,
    /// Whether members can change the chat title, photo and other settings.
    pub can_change_info: bool,
    /// Whether members can add new members or create invite links.
    pub can_invite_users: bool,
    /// Whether members can pin messages.
    pub can_pin_messages: bool,
    /// Whether members can create and edit forum topics.
    pub can_manage_topics: bool,
}

impl ChatPermissions {
    /// Every permission granted.
    pub const ALL: ChatPermissions = ChatPermissions {
        can_send_messages: true,
        can_send_audios: true,
        can_send_documents: true,
        can_send_photos: true,
        can_send_videos: true,
        can_send_video_notes: true,
        can_send_voice_notes: true,
        can_send_polls: true,
        can_send_other_messages: true,
        can_add_web_page_previews: true,
        can_react_to_messages: true,
        can_edit_tag: true,
        can_change_info: true,
        can_invite_users: true,
        can_pin_messages: true,
        can_manage_topics: true,
    };

    /// Allows nothing, like `Default`; grant permissions with the setters or start from [`ALL`](Self::ALL) and revoke.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether members can send plain text messages.
    #[must_use]
    pub fn can_send_messages(mut self, can_send_messages: bool) -> Self {
        self.can_send_messages = can_send_messages;
        self
    }

    /// Sets whether members can send music files.
    #[must_use]
    pub fn can_send_audios(mut self, can_send_audios: bool) -> Self {
        self.can_send_audios = can_send_audios;
        self
    }

    /// Sets whether members can send general files.
    #[must_use]
    pub fn can_send_documents(mut self, can_send_documents: bool) -> Self {
        self.can_send_documents = can_send_documents;
        self
    }

    /// Sets whether members can send photos.
    #[must_use]
    pub fn can_send_photos(mut self, can_send_photos: bool) -> Self {
        self.can_send_photos = can_send_photos;
        self
    }

    /// Sets whether members can send videos.
    #[must_use]
    pub fn can_send_videos(mut self, can_send_videos: bool) -> Self {
        self.can_send_videos = can_send_videos;
        self
    }

    /// Sets whether members can send round video messages.
    #[must_use]
    pub fn can_send_video_notes(mut self, can_send_video_notes: bool) -> Self {
        self.can_send_video_notes = can_send_video_notes;
        self
    }

    /// Sets whether members can send voice messages.
    #[must_use]
    pub fn can_send_voice_notes(mut self, can_send_voice_notes: bool) -> Self {
        self.can_send_voice_notes = can_send_voice_notes;
        self
    }

    /// Sets whether members can send polls.
    #[must_use]
    pub fn can_send_polls(mut self, can_send_polls: bool) -> Self {
        self.can_send_polls = can_send_polls;
        self
    }

    /// Sets whether members can send stickers, GIFs and games, and use inline bots.
    #[must_use]
    pub fn can_send_other_messages(mut self, can_send_other_messages: bool) -> Self {
        self.can_send_other_messages = can_send_other_messages;
        self
    }

    /// Sets whether members can embed link previews in messages.
    #[must_use]
    pub fn can_add_web_page_previews(mut self, can_add_web_page_previews: bool) -> Self {
        self.can_add_web_page_previews = can_add_web_page_previews;
        self
    }

    /// Sets whether members can react to messages.
    #[must_use]
    pub fn can_react_to_messages(mut self, can_react_to_messages: bool) -> Self {
        self.can_react_to_messages = can_react_to_messages;
        self
    }

    /// Sets whether members can edit their own member tag (their rank).
    #[must_use]
    pub fn can_edit_tag(mut self, can_edit_tag: bool) -> Self {
        self.can_edit_tag = can_edit_tag;
        self
    }

    /// Sets whether members can change the chat title, photo and other settings.
    #[must_use]
    pub fn can_change_info(mut self, can_change_info: bool) -> Self {
        self.can_change_info = can_change_info;
        self
    }

    /// Sets whether members can add new members or create invite links.
    #[must_use]
    pub fn can_invite_users(mut self, can_invite_users: bool) -> Self {
        self.can_invite_users = can_invite_users;
        self
    }

    /// Sets whether members can pin messages.
    #[must_use]
    pub fn can_pin_messages(mut self, can_pin_messages: bool) -> Self {
        self.can_pin_messages = can_pin_messages;
        self
    }

    /// Sets whether members can create and edit forum topics.
    #[must_use]
    pub fn can_manage_topics(mut self, can_manage_topics: bool) -> Self {
        self.can_manage_topics = can_manage_topics;
        self
    }

    pub(crate) fn parse(rights: &base::ChatBannedRights) -> ChatPermissions {
        let base::ChatBannedRights::ChatBannedRights(r) = rights;
        // Older clients still set the catch-all send_messages / send_media bans.
        let send = !r.send_messages;
        let media = send && !r.send_media;
        ChatPermissions {
            can_send_messages: send && !r.send_plain,
            can_send_audios: media && !r.send_audios,
            can_send_documents: media && !r.send_docs,
            can_send_photos: media && !r.send_photos,
            can_send_videos: media && !r.send_videos,
            can_send_video_notes: media && !r.send_roundvideos,
            can_send_voice_notes: media && !r.send_voices,
            can_send_polls: send && !r.send_polls,
            can_send_other_messages: media && !(r.send_stickers || r.send_gifs || r.send_games || r.send_inline),
            can_add_web_page_previews: send && !r.embed_links,
            can_react_to_messages: !r.send_reactions,
            can_edit_tag: !r.edit_rank,
            can_change_info: !r.change_info,
            can_invite_users: !r.invite_users,
            can_pin_messages: !r.pin_messages,
            can_manage_topics: !r.manage_topics,
        }
    }

    pub(crate) fn write(&self, until_date: i32) -> types::ChatBannedRights {
        let other = !self.can_send_other_messages;
        types::ChatBannedRights {
            view_messages: false,
            send_messages: false,
            send_media: false,
            send_stickers: other,
            send_gifs: other,
            send_games: other,
            send_inline: other,
            embed_links: !self.can_add_web_page_previews,
            send_polls: !self.can_send_polls,
            change_info: !self.can_change_info,
            invite_users: !self.can_invite_users,
            pin_messages: !self.can_pin_messages,
            manage_topics: !self.can_manage_topics,
            send_photos: !self.can_send_photos,
            send_videos: !self.can_send_videos,
            send_roundvideos: !self.can_send_video_notes,
            send_audios: !self.can_send_audios,
            send_voices: !self.can_send_voice_notes,
            send_docs: !self.can_send_documents,
            send_plain: !self.can_send_messages,
            edit_rank: !self.can_edit_tag,
            send_reactions: !self.can_react_to_messages,
            manage_linked_peers: false,
            until_date,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatPermissions;
    use crate::raw::{Serializable, base, types};

    const ALL: ChatPermissions = ChatPermissions::ALL;

    fn flags(rights: &types::ChatBannedRights) -> u32 {
        u32::from_le_bytes(rights.to_bytes()[4..8].try_into().unwrap())
    }

    fn round_trip(permissions: ChatPermissions) -> ChatPermissions {
        ChatPermissions::parse(&base::ChatBannedRights::ChatBannedRights(Box::new(permissions.write(0))))
    }

    #[test]
    fn maps_to_banned_rights_flags() {
        assert_eq!(flags(&ALL.write(0)), 0);
        assert_eq!(flags(&ChatPermissions { can_react_to_messages: false, ..ALL }.write(0)), 1 << 27);
        assert_eq!(flags(&ChatPermissions { can_send_messages: false, ..ALL }.write(0)), 1 << 25);
        assert_eq!(flags(&ChatPermissions { can_edit_tag: false, ..ALL }.write(0)), 1 << 26);
        assert_eq!(flags(&ChatPermissions { can_send_other_messages: false, ..ALL }.write(0)), 0b111_1000);
        assert_eq!(flags(&ChatPermissions::default().write(0)), 0x0ffe_85f8);
        assert_eq!(ChatPermissions::default().write(1_700_000_000).until_date, 1_700_000_000);
    }

    #[test]
    fn parses_back_and_honours_catch_all_bans() {
        assert_eq!(round_trip(ALL), ALL);
        assert_eq!(round_trip(ChatPermissions::default()), ChatPermissions::default());
        let only_reactions = ChatPermissions { can_react_to_messages: true, ..Default::default() };
        assert_eq!(round_trip(only_reactions), only_reactions);

        let mut rights = ALL.write(0);
        rights.send_media = true;
        let p = ChatPermissions::parse(&base::ChatBannedRights::ChatBannedRights(Box::new(rights)));
        assert_eq!((p.can_send_messages, p.can_send_photos, p.can_send_other_messages), (true, false, false));
    }

    #[test]
    fn builds_from_new_and_setters() {
        assert_eq!(ChatPermissions::new(), ChatPermissions::default());
        let text_only = ChatPermissions::new().can_send_messages(true);
        assert!(text_only.can_send_messages && !text_only.can_send_photos && !text_only.can_pin_messages);
        assert_eq!(round_trip(text_only), text_only);
        assert_eq!(flags(&ALL.can_react_to_messages(false).write(0)), 1 << 27);
    }
}
