use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::Chat;

/// A topic of a forum supergroup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ForumTopic {
    /// Id of the topic, which is also the `message_thread_id` of its messages.
    pub id: i32,
    /// The topic title.
    pub title: String,
    /// Color of the topic icon.
    pub icon_color: i32,
    /// Id of the custom emoji used as the icon. `None` for the default icon.
    pub icon_emoji_id: Option<i64>,
    /// Whether the topic was created without a title.
    pub is_name_implicit: bool,
    /// Unix time the topic was created.
    pub date: i32,
    /// Id of the latest message in the topic.
    pub top_message_id: i32,
    /// Number of unread messages in the topic.
    pub unread_messages_count: i32,
    /// Number of unread messages that mention the account.
    pub unread_mentions_count: i32,
    /// Number of unread reactions to the account's messages.
    pub unread_reactions_count: i32,
    /// Whether the topic is closed to new messages.
    pub is_closed: bool,
    /// Whether the topic is pinned.
    pub is_pinned: bool,
    /// Whether the topic is hidden from the topic list.
    pub is_hidden: bool,
    /// The user or channel that opened the topic.
    pub creator: Option<Chat>,
    /// Whether the account opened the topic.
    pub is_my: bool,
    /// Whether Telegram sent reduced info, with counters and read state unset.
    ///
    /// `get_forum_topics_by_id` returns the full topic.
    pub is_short: bool,
    /// Number of unread votes in the account's non-anonymous polls in this topic.
    pub unread_poll_vote_count: i32,
}

impl ForumTopic {
    pub(crate) fn parse(raw: base::ForumTopic, entities: &Entities) -> Option<ForumTopic> {
        let base::ForumTopic::ForumTopic(t) = raw else { return None };
        let t = *t;
        Some(ForumTopic {
            id: t.id,
            title: t.title,
            icon_color: t.icon_color,
            icon_emoji_id: t.icon_emoji_id,
            is_name_implicit: t.title_missing,
            date: t.date,
            top_message_id: t.top_message,
            unread_messages_count: t.unread_count,
            unread_mentions_count: t.unread_mentions_count,
            unread_reactions_count: t.unread_reactions_count,
            is_closed: t.closed,
            is_pinned: t.pinned,
            is_hidden: t.hidden,
            creator: Some(Chat::parse(&t.from_id, entities, false)),
            is_my: t.my,
            is_short: t.short,
            unread_poll_vote_count: t.unread_poll_votes_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ForumTopic;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::Chat;
    use crate::utils::{get_channel_id, get_peer};

    fn topic(from_id: base::Peer) -> base::ForumTopic {
        let notify_settings = types::PeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: None,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        };
        base::ForumTopic::ForumTopic(Box::new(types::ForumTopic {
            my: true,
            closed: false,
            pinned: false,
            short: true,
            hidden: false,
            title_missing: false,
            id: 5,
            date: 1,
            peer: get_peer(get_channel_id(9)),
            title: "t".into(),
            icon_color: 0,
            icon_emoji_id: None,
            top_message: 5,
            read_inbox_max_id: 0,
            read_outbox_max_id: 0,
            unread_count: 0,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            unread_poll_votes_count: 3,
            from_id,
            notify_settings: base::PeerNotifySettings::PeerNotifySettings(Box::new(notify_settings)),
            draft: None,
        }))
    }

    #[test]
    fn creator_is_the_user_or_channel_that_opened_it() {
        let entities = Entities::default();
        for peer in [get_peer(7), get_peer(get_channel_id(8))] {
            let parsed = ForumTopic::parse(topic(peer.clone()), &entities).unwrap();
            assert_eq!(parsed.creator, Some(Chat::parse(&peer, &entities, false)));
            assert_eq!((parsed.is_my, parsed.is_short, parsed.unread_poll_vote_count), (true, true, 3));
        }
        let channel = ForumTopic::parse(topic(get_peer(get_channel_id(8))), &entities).unwrap();
        assert_eq!(channel.creator.map(|c| c.id), Some(get_channel_id(8)));
    }
}
