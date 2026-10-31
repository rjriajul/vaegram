mod add_chat_members;
mod archive_chats;
mod ban_chat_member;
mod ban_chat_sender_chat;
mod close_forum_topic;
mod close_general_forum_topic;
mod create_channel;
mod create_forum_topic;
mod create_group;
mod create_supergroup;
mod delete_channel;
mod delete_chat_history;
mod delete_chat_photo;
mod delete_chat_sticker_set;
mod delete_chat_wallpaper;
mod delete_forum_topic;
mod delete_supergroup;
mod edit_forum_topic;
mod edit_general_forum_topic;
mod get_channel_statistics;
mod get_chat;
mod get_chat_administrators;
mod get_chat_event_log;
mod get_chat_member;
mod get_chat_members;
mod get_chat_members_count;
mod get_chat_notifications;
mod get_chat_online_count;
mod get_dialogs;
mod get_dialogs_count;
mod get_forum_topic_icon_stickers;
mod get_forum_topics;
mod get_forum_topics_by_id;
mod get_message_public_forwards;
mod get_message_statistics;
mod get_peer_colors;
mod get_peer_profile_colors;
mod get_recommended_channels;
mod get_send_as_chats;
mod get_similar_channels;
mod get_statistical_graph;
mod get_story_statistics;
mod get_supergroup_statistics;
mod hide_general_forum_topic;
mod join_chat;
mod leave_chat;
mod mark_chat_unread;
mod mute_chat;
mod pin_chat;
mod pin_forum_topic;
mod promote_chat_member;
mod reopen_forum_topic;
mod reopen_general_forum_topic;
mod restrict_chat_member;
mod restrict_sponsored_messages;
mod set_chat_accent_color;
mod set_chat_administrator_custom_title;
mod set_chat_description;
mod set_chat_member_tag;
mod set_chat_permissions;
mod set_chat_photo;
mod set_chat_profile_accent_color;
mod set_chat_protected_content;
mod set_chat_sticker_set;
mod set_chat_theme;
mod set_chat_title;
mod set_chat_ttl;
mod set_chat_username;
mod set_chat_wallpaper;
mod set_send_as_chat;
mod set_slow_mode;
mod toggle_anti_spam;
mod toggle_auto_translation;
mod toggle_forum_topics;
mod toggle_join_request;
mod toggle_join_to_send;
mod toggle_participants_hidden;
mod toggle_pre_history_hidden;
mod toggle_signatures;
mod toggle_view_forum_as_messages;
mod transfer_chat_ownership;
mod unarchive_chats;
mod unban_chat_member;
mod unban_chat_sender_chat;
mod unhide_general_forum_topic;
mod unmute_chat;
mod unpin_all_forum_topic_messages;
mod unpin_all_general_forum_topic_messages;
mod unpin_chat;
mod unpin_forum_topic;
mod update_chat_notifications;

pub use ban_chat_member::BanChatMember;
pub use create_forum_topic::CreateForumTopic;
pub use delete_chat_history::DeleteChatHistory;
pub use delete_chat_wallpaper::DeleteChatWallpaper;
pub use edit_forum_topic::EditForumTopic;
pub use get_chat_administrators::GetChatAdministrators;
pub use get_chat_event_log::GetChatEventLog;
pub use get_chat_members::GetChatMembers;
pub use get_dialogs::GetDialogs;
pub use get_forum_topics::GetForumTopics;
pub use get_message_public_forwards::GetMessagePublicForwards;
pub use restrict_chat_member::RestrictChatMember;
pub use set_chat_accent_color::SetChatAccentColor;
pub use set_chat_photo::SetChatPhoto;
pub use set_chat_wallpaper::SetChatWallpaper;
pub use update_chat_notifications::UpdateChatNotifications;

#[cfg(test)]
mod creation_tests {
    use super::add_chat_members::missing_ids;
    use super::create_channel::created_chat;
    use crate::Client;
    use crate::client::sent;
    use crate::enums::ChatType;
    use crate::errors::{Error, Result};
    use crate::raw::functions::{channels, messages};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::session::Affected;
    use crate::utils::get_channel_id;

    const GROUP: i64 = -5;

    #[tokio::test]
    async fn an_empty_member_list_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let result = client.add_chat_members(GROUP, Vec::<i64>::new(), 100).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn deleted_history_moves_the_account_pts() {
        let affected = |pts, offset| {
            let a = types::messages::AffectedHistory { pts, pts_count: 1, offset };
            Ok(base::messages::AffectedHistory::AffectedHistory(Box::new(a)).to_bytes())
        };
        let (client, _rx) = Client::scripted(vec![affected(11, 3), affected(12, 0)]);
        client.delete_chat_history(GROUP).await.unwrap();
        assert_eq!(client.0.events.take_affected(), Some(Affected { channel_id: None, pts: 12, pts_count: 2 }));
    }

    #[tokio::test]
    async fn basic_groups_and_channels_use_their_own_methods() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        let rx = &mut rx;

        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));

        assert_eq!(sent(rx, client.add_chat_members(GROUP, [7_i64], 100)).await, messages::AddChatUser::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, client.add_chat_members(channel, [7_i64], 100)).await,
            channels::InviteToChannel::CONSTRUCTOR_ID
        );
        assert_eq!(sent(rx, client.delete_chat_history(GROUP)).await, messages::DeleteHistory::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.delete_chat_history(channel)).await, channels::DeleteHistory::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_slow_mode(channel, Some(30))).await, channels::ToggleSlowMode::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.delete_supergroup(channel)).await, channels::DeleteChannel::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn channel_only_methods_refuse_basic_groups_without_a_request() {
        let (client, mut rx) = Client::recording();
        let refused = |r: Result<()>| matches!(r, Err(Error::InvalidArgument(_)));

        let slow_mode = refused(client.set_slow_mode(GROUP, None).await);
        let join_to_send = refused(client.toggle_join_to_send(GROUP, true).await);
        let username = refused(client.set_chat_username(GROUP, None).await);
        assert_eq!((slow_mode, join_to_send, username, rx.try_recv().is_err()), (true, true, true, true));
    }

    #[test]
    fn created_chat_is_read_from_the_updates() {
        let group = types::Chat {
            creator: true,
            left: false,
            deactivated: false,
            call_active: false,
            call_not_empty: false,
            noforwards: false,
            id: 5,
            title: "friends".into(),
            photo: base::ChatPhoto::ChatPhotoEmpty,
            participants_count: 2,
            date: 0,
            version: 1,
            migrated_to: None,
            admin_rights: None,
            default_banned_rights: None,
        };
        let updates = types::Updates {
            updates: Vec::new(),
            users: Vec::new(),
            chats: vec![base::Chat::ChatEmpty(types::ChatEmpty { id: 1 }), base::Chat::Chat(Box::new(group))],
            date: 0,
            seq: 0,
        };
        let chat = created_chat(&Client::offline(), base::Updates::Updates(Box::new(updates))).unwrap();
        assert_eq!((chat.id, chat.kind, chat.title.as_deref()), (GROUP, ChatType::Group, Some("friends")));
    }

    #[test]
    fn users_kept_out_by_privacy_are_reported() {
        let missing = |user_id| {
            let m =
                types::MissingInvitee { premium_would_allow_invite: false, premium_required_for_pm: false, user_id };
            base::MissingInvitee::MissingInvitee(Box::new(m))
        };
        let invited = types::messages::InvitedUsers {
            updates: base::Updates::UpdatesTooLong,
            missing_invitees: vec![missing(7), missing(8)],
        };
        assert_eq!(missing_ids(&invited), [7, 8]);
    }
}

#[cfg(test)]
mod dialog_tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::{Error, Result};
    use crate::raw::Identifiable;
    use crate::raw::functions::{folders, messages};

    const GROUP: i64 = -5;

    async fn sent(rx: &mut UnboundedReceiver<Vec<u8>>, call: impl Future) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    fn word(body: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(body[at..at + 4].try_into().unwrap())
    }

    #[tokio::test]
    async fn dialog_toggles_set_the_right_flags() {
        let (client, mut rx) = Client::recording();
        let rx = &mut rx;

        let pin = sent(rx, client.pin_chat(GROUP)).await;
        assert_eq!((word(&pin, 0), word(&pin, 4)), (messages::ToggleDialogPin::CONSTRUCTOR_ID, 1));
        let unpin = sent(rx, client.unpin_chat(GROUP)).await;
        assert_eq!((word(&unpin, 0), word(&unpin, 4)), (messages::ToggleDialogPin::CONSTRUCTOR_ID, 0));
        let unread = sent(rx, client.mark_chat_unread(GROUP)).await;
        assert_eq!((word(&unread, 0), word(&unread, 4)), (messages::MarkDialogUnread::CONSTRUCTOR_ID, 1));

        let archive = sent(rx, client.archive_chats([GROUP, -6])).await;
        assert_eq!(word(&archive, 0), folders::EditPeerFolders::CONSTRUCTOR_ID);
        assert_eq!((word(&archive, 8), word(&archive, archive.len() - 4)), (2, 1));
        let unarchive = sent(rx, client.unarchive_chats([GROUP])).await;
        assert_eq!(word(&unarchive, unarchive.len() - 4), 0);

        let count = sent(rx, client.get_dialogs_count(true)).await;
        assert_eq!((word(&count, 0), word(&count, 4)), (messages::GetDialogs::CONSTRUCTOR_ID, 2));
    }

    #[tokio::test]
    async fn moving_no_chats_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let archived = client.archive_chats(Vec::<i64>::new()).await;
        let unarchived = client.unarchive_chats(Vec::<i64>::new()).await;
        let refused = |r: &Result<()>| matches!(r, Err(Error::InvalidArgument(_)));
        assert!(refused(&archived) && refused(&unarchived), "{archived:?} {unarchived:?}");
        assert!(rx.try_recv().is_err());
    }
}

#[cfg(test)]
mod statistics_tests {
    use crate::client::sent;
    use crate::raw::functions::{channels, stats};
    use crate::raw::{Identifiable, base, types};
    use crate::types::ChatEventFilter;
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    #[tokio::test]
    async fn statistics_and_admin_log_hit_the_right_calls() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        let rx = &mut rx;

        assert_eq!(sent(rx, client.get_channel_statistics(channel)).await, stats::GetBroadcastStats::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_supergroup_statistics(channel)).await, stats::GetMegagroupStats::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_message_statistics(channel, 1)).await, stats::GetMessageStats::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_story_statistics(channel, 1)).await, stats::GetStoryStats::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_statistical_graph("t", None)).await, stats::LoadAsyncGraph::CONSTRUCTOR_ID);
        let mut forwards = client.get_message_public_forwards(channel, 1);
        assert_eq!(sent(rx, forwards.next()).await, stats::GetMessagePublicForwards::CONSTRUCTOR_ID);
        let filter = ChatEventFilter { deleted_messages: true, ..Default::default() };
        let mut log = client.get_chat_event_log(channel).filter(filter).query("spam");
        assert_eq!(sent(rx, log.next()).await, channels::GetAdminLog::CONSTRUCTOR_ID);

        let statistics = client.get_channel_statistics(-5).await.map(drop);
        let log = client.get_chat_event_log(-5).next().await.map(drop);
        for result in [statistics, log] {
            assert!(matches!(result, Err(Error::InvalidArgument(_))));
        }
        assert!(rx.try_recv().is_err());
    }
}

#[cfg(test)]
mod appearance_tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::raw::Identifiable;
    use crate::raw::functions::{account, channels, messages};
    use crate::raw::{base, types};
    use crate::types::ChatBackground;
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    const GROUP: i64 = -5;

    async fn flags(rx: &mut UnboundedReceiver<Vec<u8>>, call: impl IntoFuture) -> (u32, u32) {
        let _ = call.await;
        let body = rx.try_recv().expect("no request sent");
        let word = |at: usize| u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
        (word(0), word(4))
    }

    #[tokio::test]
    async fn wallpaper_calls_set_their_flags() {
        let (client, mut rx) = Client::recording();
        let rx = &mut rx;
        let fill = ChatBackground { background_color: Some(0xffffff), ..Default::default() };
        let id = messages::SetChatWallPaper::CONSTRUCTOR_ID;

        assert_eq!(flags(rx, client.set_chat_wallpaper(GROUP, &fill)).await, (id, 0b101));
        assert_eq!(flags(rx, client.set_chat_wallpaper(GROUP, &fill).for_both()).await, (id, 0b1101));
        assert_eq!(flags(rx, client.delete_chat_wallpaper(GROUP)).await, (id, 0));
        assert_eq!(flags(rx, client.delete_chat_wallpaper(GROUP).restore_previous()).await, (id, 0b10000));
    }

    #[tokio::test]
    async fn accent_colors_go_to_the_account_or_the_channel() {
        let (client, mut rx) = Client::recording();
        let rx = &mut rx;
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        client.0.me.lock().unwrap().id = 7;
        let (me, channel_color) = (account::UpdateColor::CONSTRUCTOR_ID, channels::UpdateColor::CONSTRUCTOR_ID);

        assert_eq!(flags(rx, client.set_chat_accent_color(7, Some(3))).await, (me, 0b100));
        assert_eq!(flags(rx, client.set_chat_accent_color(7, None)).await, (me, 0));
        assert_eq!(flags(rx, client.set_chat_profile_accent_color(7, Some(3))).await, (me, 0b110));
        let emoji = client.set_chat_accent_color(channel, Some(3)).background_custom_emoji_id(5);
        assert_eq!(flags(rx, emoji).await, (channel_color, 0b101));

        let refused = client.set_chat_accent_color(GROUP, Some(3)).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}

#[cfg(test)]
mod settings_tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::functions::{channels, messages};
    use crate::raw::{Serializable, base, types};
    use crate::utils::get_channel_id;

    const GROUP: i64 = -5;
    const USER: i64 = 7;

    fn channel() -> base::InputChannel {
        base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 1 }))
    }

    fn channel_peer() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }))
    }

    fn user_peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }))
    }

    fn settings_client(replies: Vec<Result<Vec<u8>>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies);
        client.0.peers.lock().unwrap().extend([(get_channel_id(9), channel_peer()), (USER, user_peer())]);
        (client, rx)
    }

    async fn body<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    fn rpc(name: &str) -> Error {
        Error::Rpc(RpcError::new(400, name))
    }

    async fn toggles_refused(client: &Client, chat: i64) -> [bool; 6] {
        let refused = |r: Result<()>| matches!(r, Err(Error::InvalidArgument(_)));
        [
            refused(client.toggle_anti_spam(chat, true).await),
            refused(client.toggle_auto_translation(chat, true).await),
            refused(client.toggle_participants_hidden(chat, true).await),
            refused(client.toggle_pre_history_hidden(chat, true).await),
            refused(client.toggle_view_forum_as_messages(chat, true).await),
            refused(client.restrict_sponsored_messages(chat, true).await),
        ]
    }

    #[tokio::test]
    async fn admin_toggles_send_their_requests() {
        let (client, mut rx) = settings_client(Vec::new());
        let (rx, chat, enabled) = (&mut rx, get_channel_id(9), true);
        let anti_spam = channels::ToggleAntiSpam { channel: channel(), enabled };
        assert_eq!(body(rx, client.toggle_anti_spam(chat, true)).await, anti_spam.to_bytes());
        let translation = channels::ToggleAutotranslation { channel: channel(), enabled };
        assert_eq!(body(rx, client.toggle_auto_translation(chat, true)).await, translation.to_bytes());
        let hidden = channels::ToggleParticipantsHidden { channel: channel(), enabled };
        assert_eq!(body(rx, client.toggle_participants_hidden(chat, true)).await, hidden.to_bytes());
        let history = channels::TogglePreHistoryHidden { channel: channel(), enabled };
        assert_eq!(body(rx, client.toggle_pre_history_hidden(chat, true)).await, history.to_bytes());
        let forum = channels::ToggleViewForumAsMessages { channel: channel(), enabled };
        assert_eq!(body(rx, client.toggle_view_forum_as_messages(chat, true)).await, forum.to_bytes());
        let sponsored = channels::RestrictSponsoredMessages { channel: channel(), restricted: true };
        assert_eq!(body(rx, client.restrict_sponsored_messages(chat, true)).await, sponsored.to_bytes());

        let ttl = messages::SetHistoryTTL { peer: channel_peer(), period: 86400 };
        assert_eq!(body(rx, client.set_chat_ttl(chat, 86400)).await, ttl.to_bytes());
        let ttl = messages::SetHistoryTTL { peer: user_peer(), period: 0 };
        assert_eq!(body(rx, client.set_chat_ttl(USER, 0)).await, ttl.to_bytes());
    }

    #[tokio::test]
    async fn admin_toggles_refuse_basic_groups_without_a_request() {
        let (client, mut rx) = settings_client(Vec::new());
        assert_eq!(toggles_refused(&client, GROUP).await, [true; 6]);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn admin_toggles_refuse_a_user() {
        let (client, mut rx) = settings_client(Vec::new());
        assert_eq!(toggles_refused(&client, USER).await, [true; 6]);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn telegram_errors_pass_through() {
        let (client, _rx) = settings_client(vec![Err(rpc("TTL_PERIOD_INVALID")), Err(rpc("CHAT_NOT_MODIFIED"))]);
        let ttl = client.set_chat_ttl(get_channel_id(9), 7).await;
        assert!(matches!(&ttl, Err(Error::Rpc(e)) if e.name == "TTL_PERIOD_INVALID"), "{ttl:?}");
        let toggle = client.toggle_anti_spam(get_channel_id(9), true).await;
        assert!(matches!(&toggle, Err(Error::Rpc(e)) if e.name == "CHAT_NOT_MODIFIED"), "{toggle:?}");
    }
}

#[cfg(test)]
mod forum_topic_tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::{ForumTopic, Message};
    use crate::utils::{get_channel_id, get_peer};

    const USER: i64 = 7;

    fn channel() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }))
    }

    fn user_peer() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }))
    }

    fn topic_client(replies: Vec<Result<Vec<u8>>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies);
        client.0.peers.lock().unwrap().extend([(get_channel_id(9), channel()), (USER, user_peer())]);
        (client, rx)
    }

    fn user(id: i64) -> base::User {
        let mut buf = Vec::new();
        // Only access_hash is set.
        for word in [1u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn topic(id: i32, date: i32, top_message: i32) -> base::ForumTopic {
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
        let topic = types::ForumTopic {
            my: false,
            closed: false,
            pinned: false,
            short: false,
            hidden: false,
            title_missing: false,
            id,
            date,
            peer: get_peer(get_channel_id(9)),
            title: format!("topic {id}"),
            icon_color: 0x6FB9F0,
            icon_emoji_id: None,
            top_message,
            read_inbox_max_id: 0,
            read_outbox_max_id: 0,
            unread_count: 0,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            unread_poll_votes_count: 0,
            from_id: get_peer(USER),
            notify_settings: base::PeerNotifySettings::PeerNotifySettings(Box::new(notify_settings)),
            draft: None,
        };
        base::ForumTopic::ForumTopic(Box::new(topic))
    }

    fn deleted(id: i32) -> base::ForumTopic {
        base::ForumTopic::ForumTopicDeleted(types::ForumTopicDeleted { id })
    }

    fn message(id: i32, date: i32) -> base::Message {
        base::Message::Message(Box::new(Message::bare(id, get_peer(get_channel_id(9)), date)))
    }

    fn page(topics: Vec<base::ForumTopic>, messages: Vec<base::Message>, order_by_create_date: bool) -> Vec<u8> {
        let page = types::messages::ForumTopics {
            order_by_create_date,
            count: 1000,
            topics,
            messages,
            chats: Vec::new(),
            users: vec![user(8)],
            pts: 1,
        };
        base::messages::ForumTopics::ForumTopics(Box::new(page)).to_bytes()
    }

    // The last topic is 100, created at 110 with top message 1100.
    fn full() -> Vec<base::ForumTopic> {
        (1..=100).map(|id| topic(id, 10 + id, 1000 + id)).collect()
    }

    fn list(offset_date: i32, offset_id: i32, offset_topic: i32, limit: i32) -> Vec<u8> {
        messages::GetForumTopics { peer: channel(), q: None, offset_date, offset_id, offset_topic, limit }.to_bytes()
    }

    async fn request_after(first: Vec<u8>) -> Vec<u8> {
        let (client, mut rx) = topic_client(vec![Ok(first)]);
        let mut topics = client.get_forum_topics(get_channel_id(9));
        while let Ok(Some(_)) = topics.next().await {}
        rx.try_recv().unwrap();
        rx.try_recv().expect("no second request")
    }

    #[test]
    fn parses_topic_flags_and_counts() {
        let base::ForumTopic::ForumTopic(mut raw) = topic(5, 100, 40) else { unreachable!() };
        (raw.closed, raw.pinned, raw.hidden, raw.title_missing) = (true, true, true, true);
        (raw.unread_count, raw.unread_mentions_count, raw.unread_reactions_count) = (3, 2, 1);
        raw.icon_emoji_id = Some(42);
        let expected = ForumTopic {
            id: 5,
            title: "topic 5".into(),
            icon_color: 0x6FB9F0,
            icon_emoji_id: Some(42),
            is_name_implicit: true,
            date: 100,
            top_message_id: 40,
            unread_messages_count: 3,
            unread_mentions_count: 2,
            unread_reactions_count: 1,
            is_closed: true,
            is_pinned: true,
            is_hidden: true,
        };
        assert_eq!(ForumTopic::parse(base::ForumTopic::ForumTopic(raw)), Some(expected));
    }

    #[test]
    fn deleted_topic_parses_to_none() {
        assert_eq!(ForumTopic::parse(deleted(5)), None);
    }

    #[tokio::test]
    async fn lists_topics_until_a_short_page() {
        let first = page(full(), vec![message(1100, 77)], false);
        let (client, mut rx) = topic_client(vec![Ok(first), Ok(page(vec![topic(101, 5, 1101)], Vec::new(), false))]);
        let mut topics = client.get_forum_topics(get_channel_id(9));
        let mut ids = Vec::new();
        while let Some(topic) = topics.next().await.unwrap() {
            ids.push(topic.id);
        }
        assert_eq!(ids, (1..=101).collect::<Vec<_>>());
        assert_eq!(rx.try_recv().unwrap(), list(0, 0, 0, 100));
        assert_eq!(rx.try_recv().unwrap(), list(77, 1100, 100, 100));
        assert!(rx.try_recv().is_err());
        assert!(client.0.peers.lock().unwrap().contains_key(&8));

        let _ = client.get_forum_topics(get_channel_id(9)).limit(3).next().await;
        assert_eq!(rx.try_recv().unwrap(), list(0, 0, 0, 3));
    }

    #[tokio::test]
    async fn the_cursor_falls_back_to_the_topic_date() {
        let by_creation = page(full(), vec![message(1100, 77)], true);
        assert_eq!(request_after(by_creation).await, list(110, 1100, 100, 100));
        let top_message_missing = page(full(), Vec::new(), false);
        assert_eq!(request_after(top_message_missing).await, list(110, 1100, 100, 100));
    }

    #[tokio::test]
    async fn deleted_topics_are_left_out_of_the_list_and_the_cursor() {
        let first = page(vec![topic(5, 50, 40), topic(6, 60, 41), deleted(7)], vec![message(41, 90)], false);
        let (client, mut rx) = topic_client(vec![Ok(first)]);
        let mut topics = client.get_forum_topics(get_channel_id(9)).limit(3);
        let mut ids = Vec::new();
        while let Ok(Some(topic)) = topics.next().await {
            ids.push(topic.id);
        }
        assert_eq!(ids, [5, 6]);
        rx.try_recv().unwrap();
        assert_eq!(rx.try_recv().unwrap(), list(90, 41, 6, 1));
    }

    #[tokio::test]
    async fn a_repeated_page_ends_the_list() {
        let (client, mut rx) = topic_client((0..3).map(|_| Ok(page(full(), Vec::new(), false))).collect());
        let mut topics = client.get_forum_topics(get_channel_id(9));
        let mut count = 0;
        while topics.next().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 200);
        assert_eq!((rx.try_recv().is_ok(), rx.try_recv().is_ok(), rx.try_recv().is_err()), (true, true, true));
    }

    #[tokio::test]
    async fn full_pages_of_deleted_topics_are_retried_whole_then_refused() {
        let all_deleted = |n| page((1..=n).map(deleted).collect(), Vec::new(), false);
        let (client, mut rx) = topic_client(vec![Ok(all_deleted(2)), Ok(all_deleted(100))]);
        let result = client.get_forum_topics(get_channel_id(9)).limit(2).next().await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
        assert_eq!(rx.try_recv().unwrap(), list(0, 0, 0, 2));
        assert_eq!(rx.try_recv().unwrap(), list(0, 0, 0, 100));
    }

    #[tokio::test]
    async fn looks_topics_up_by_id() {
        let (client, mut rx) = topic_client(vec![Ok(page(vec![topic(5, 50, 40), deleted(6)], Vec::new(), false))]);
        let refused = client.get_forum_topics_by_id(get_channel_id(9), &[]).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(rx.try_recv().is_err());

        let topics = client.get_forum_topics_by_id(get_channel_id(9), &[5, 6]).await.unwrap();
        assert_eq!(topics.iter().map(|t| t.id).collect::<Vec<_>>(), [5]);
        let request = messages::GetForumTopicsByID { peer: channel(), topics: vec![5, 6] };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert!(client.0.peers.lock().unwrap().contains_key(&8));
    }

    #[tokio::test]
    async fn pins_and_unpins_topics_in_channels_and_bot_chats() {
        let updates = || {
            let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
            Ok(base::Updates::Updates(Box::new(updates)).to_bytes())
        };
        let (client, mut rx) = topic_client((0..4).map(|_| updates()).collect());
        let pin = |peer, pinned| messages::UpdatePinnedForumTopic { peer, topic_id: 5, pinned }.to_bytes();
        client.pin_forum_topic(get_channel_id(9), 5).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), pin(channel(), true));
        client.unpin_forum_topic(get_channel_id(9), 5).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), pin(channel(), false));
        client.pin_forum_topic(USER, 5).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), pin(user_peer(), true));
        client.unpin_forum_topic(USER, 5).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), pin(user_peer(), false));
    }

    #[tokio::test]
    async fn a_refused_pin_returns_the_telegram_error() {
        let (client, _rx) = topic_client(vec![Err(Error::Rpc(RpcError::new(400, "TOPIC_ID_INVALID")))]);
        let result = client.pin_forum_topic(get_channel_id(9), 5).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "TOPIC_ID_INVALID"), "{result:?}");
    }
}
