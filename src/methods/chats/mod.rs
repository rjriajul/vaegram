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
mod delete_user_history;
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
mod get_chat_settings;
mod get_dialogs;
mod get_dialogs_count;
mod get_direct_messages_topics_by_id;
mod get_forum_topic_icon_stickers;
mod get_forum_topics;
mod get_forum_topics_by_id;
mod get_message_public_forwards;
mod get_message_statistics;
mod get_peer_colors;
mod get_peer_profile_colors;
mod get_personal_channels;
mod get_poll_statistics;
mod get_recommended_channels;
mod get_send_as_chats;
mod get_similar_channels;
mod get_statistical_graph;
mod get_story_statistics;
mod get_suitable_discussion_chats;
mod get_supergroup_statistics;
mod get_top_chats;
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
mod set_chat_direct_messages_group;
mod set_chat_discussion_group;
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
mod set_main_profile_tab;
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
    use crate::enums::{ProfileTab, TopChatCategory};
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::functions::{account, channels, contacts, messages};
    use crate::raw::{Serializable, base, types};
    use crate::session::Affected;
    use crate::types::{ChatSettings, User};
    use crate::utils::{get_channel_id, get_peer};

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

    fn affected(pts: i32, pts_count: i32, offset: i32) -> Result<Vec<u8>> {
        let affected = types::messages::AffectedHistory { pts, pts_count, offset };
        Ok(base::messages::AffectedHistory::AffectedHistory(Box::new(affected)).to_bytes())
    }

    fn delete_history(participant: base::InputPeer) -> Vec<u8> {
        channels::DeleteParticipantHistory { channel: channel(), participant }.to_bytes()
    }

    #[tokio::test]
    async fn member_history_repeats_batches_and_moves_the_channel_pts() {
        let (client, mut rx) = settings_client(vec![affected(11, 1, 3), affected(12, 1, 0)]);
        client.delete_user_history(get_channel_id(9), USER).await.unwrap();
        let request = delete_history(user_peer());
        assert_eq!((rx.try_recv().unwrap(), rx.try_recv().unwrap()), (request.clone(), request));
        assert!(rx.try_recv().is_err());
        assert_eq!(client.0.events.take_affected(), Some(Affected { channel_id: Some(9), pts: 12, pts_count: 2 }));
    }

    #[tokio::test]
    async fn member_history_with_nothing_deleted_moves_no_pts() {
        // Telegram answers pts_count 0 when it found nothing it could delete.
        let (client, _rx) = settings_client(vec![affected(11, 0, 0)]);
        client.delete_user_history(get_channel_id(9), USER).await.unwrap();
        assert_eq!(client.0.events.take_affected(), None);
    }

    #[tokio::test]
    async fn member_history_refuses_basic_groups_and_users() {
        let (client, mut rx) = settings_client(Vec::new());
        let group = client.delete_user_history(GROUP, USER).await;
        let user = client.delete_user_history(USER, USER).await;
        for result in [group, user] {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn member_history_of_the_account_itself_sends_self() {
        let (client, mut rx) = settings_client(Vec::new());
        let sent = body(&mut rx, client.delete_user_history(get_channel_id(9), "me")).await;
        assert_eq!(sent, delete_history(base::InputPeer::InputPeerSelf));
    }

    fn group_channel() -> base::InputChannel {
        base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 10, access_hash: 3 }))
    }

    fn chats_reply(chats: Vec<base::Chat>) -> Result<Vec<u8>> {
        Ok(base::messages::Chats::Chats(Box::new(types::messages::Chats { chats })).to_bytes())
    }

    #[tokio::test]
    async fn discussion_group_links_and_unlinks() {
        let (client, mut rx) = settings_client(Vec::new());
        let group = types::InputPeerChannel { channel_id: 10, access_hash: 3 };
        client.0.peers.lock().unwrap().insert(get_channel_id(10), base::InputPeer::InputPeerChannel(Box::new(group)));
        let (rx, chat) = (&mut rx, get_channel_id(9));

        let linked = channels::SetDiscussionGroup { broadcast: channel(), group: group_channel() };
        let link = client.set_chat_discussion_group(chat, Some(get_channel_id(10).into()));
        assert_eq!(body(rx, link).await, linked.to_bytes());
        let unlinked =
            channels::SetDiscussionGroup { broadcast: channel(), group: base::InputChannel::InputChannelEmpty };
        assert_eq!(body(rx, client.set_chat_discussion_group(chat, None)).await, unlinked.to_bytes());
    }

    #[tokio::test]
    async fn discussion_group_refuses_a_basic_group() {
        let (client, mut rx) = settings_client(Vec::new());
        let result = client.set_chat_discussion_group(get_channel_id(9), Some(GROUP.into())).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn personal_channels_ask_only_for_personal_ones() {
        let channel = types::ChannelForbidden {
            broadcast: true,
            megagroup: false,
            monoforum: false,
            id: 9,
            access_hash: 1,
            title: "channel".into(),
            until_date: None,
        };
        let group = types::ChatForbidden { id: 5, title: "group".into() };
        let chats = vec![base::Chat::ChannelForbidden(Box::new(channel)), base::Chat::ChatForbidden(Box::new(group))];
        let slice = types::messages::ChatsSlice { count: 2, chats };
        let slice = Ok(base::messages::Chats::ChatsSlice(Box::new(slice)).to_bytes());
        let (client, mut rx) = settings_client(vec![slice, chats_reply(Vec::new())]);

        let found = client.get_personal_channels().await.unwrap();
        assert_eq!(found.iter().map(|c| c.id).collect::<Vec<_>>(), [get_channel_id(9), GROUP]);
        let request = channels::GetAdminedPublicChannels {
            by_location: false,
            check_limit: false,
            for_personal: true,
            for_community_peer: false,
        };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        // An account that owns no channel gets an empty list.
        assert!(client.get_personal_channels().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn suitable_discussion_chats_send_their_request() {
        let (client, mut rx) = settings_client(vec![chats_reply(Vec::new())]);
        assert!(client.get_suitable_discussion_chats().await.unwrap().is_empty());
        assert_eq!(rx.try_recv().unwrap(), channels::GetGroupsForDiscussion {}.to_bytes());
    }

    #[tokio::test]
    async fn main_profile_tab_goes_to_the_account_or_the_channel() {
        let (client, mut rx) = Client::recording();
        let other = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 8, access_hash: 2 }));
        client.0.peers.lock().unwrap().extend([(get_channel_id(9), channel_peer()), (8, other)]);
        client.0.me.lock().unwrap().id = USER;
        let rx = &mut rx;

        let gifts = account::SetMainProfileTab { tab: base::ProfileTab::ProfileTabGifts }.to_bytes();
        assert_eq!(body(rx, client.set_main_profile_tab("me", ProfileTab::Gifts)).await, gifts);
        assert_eq!(body(rx, client.set_main_profile_tab(USER, ProfileTab::Gifts)).await, gifts);
        let media = channels::SetMainProfileTab { channel: channel(), tab: base::ProfileTab::ProfileTabMedia };
        assert_eq!(body(rx, client.set_main_profile_tab(get_channel_id(9), ProfileTab::Media)).await, media.to_bytes());

        for chat in [GROUP, 8] {
            let result = client.set_main_profile_tab(chat, ProfileTab::Media).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn direct_messages_settings_send_the_price() {
        let (client, mut rx) = settings_client(Vec::new());
        let (rx, chat) = (&mut rx, get_channel_id(9));
        let price = |broadcast_messages_allowed, send_paid_messages_stars| {
            let request = channels::UpdatePaidMessagesPrice {
                broadcast_messages_allowed,
                channel: channel(),
                send_paid_messages_stars,
            };
            request.to_bytes()
        };
        assert_eq!(body(rx, client.set_chat_direct_messages_group(chat, true, 10)).await, price(true, 10));
        assert_eq!(body(rx, client.set_chat_direct_messages_group(chat, false, 0)).await, price(false, 0));
    }

    #[tokio::test]
    async fn direct_messages_settings_refuse_negative_stars_and_basic_groups() {
        let (client, mut rx) = settings_client(Vec::new());
        let negative = client.set_chat_direct_messages_group(get_channel_id(9), true, -1).await;
        let group = client.set_chat_direct_messages_group(GROUP, true, 0).await;
        for result in [negative, group] {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn direct_messages_settings_return_the_telegram_error() {
        let (client, _rx) = settings_client(vec![Err(rpc("CHANNEL_MONOFORUM_UNSUPPORTED"))]);
        let result = client.set_chat_direct_messages_group(get_channel_id(9), true, 0).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "CHANNEL_MONOFORUM_UNSUPPORTED"), "{result:?}");
    }

    fn named_user(id: i64) -> base::User {
        let mut buf = Vec::new();
        // Only access_hash and first_name are set.
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        String::from("Ann").serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn monoforum_dialog(user_id: i64) -> base::SavedDialog {
        let dialog = types::MonoForumDialog {
            unread_mark: true,
            nopaid_messages_exception: true,
            peer: get_peer(user_id),
            top_message: 40,
            read_inbox_max_id: 30,
            read_outbox_max_id: 31,
            unread_count: 3,
            unread_reactions_count: 2,
            draft: None,
        };
        base::SavedDialog::MonoForumDialog(Box::new(dialog))
    }

    fn saved_dialogs(dialogs: Vec<base::SavedDialog>, users: Vec<base::User>) -> Result<Vec<u8>> {
        let page = types::messages::SavedDialogs { dialogs, messages: Vec::new(), chats: Vec::new(), users };
        Ok(base::messages::SavedDialogs::SavedDialogs(Box::new(page)).to_bytes())
    }

    #[tokio::test]
    async fn direct_messages_topics_are_looked_up_by_user() {
        let saved = types::SavedDialog { pinned: false, peer: get_peer(8), top_message: 1 };
        let dialogs = vec![monoforum_dialog(USER), base::SavedDialog::SavedDialog(Box::new(saved))];
        let (client, mut rx) = settings_client(vec![saved_dialogs(dialogs, vec![named_user(USER)])]);
        let refused = client.get_direct_messages_topics_by_id(get_channel_id(9), &[]).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(rx.try_recv().is_err());

        let topics = client.get_direct_messages_topics_by_id(get_channel_id(9), &[USER]).await.unwrap();
        let request = messages::GetSavedDialogsByID { parent_peer: Some(channel_peer()), ids: vec![user_peer()] };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert_eq!(topics.len(), 1);
        let topic = &topics[0];
        let user = topic.topic.user.as_ref().map(|u| u.first_name.as_str());
        assert_eq!((topic.topic.topic_id, user), (USER, Some("Ann")));
        let counts = (topic.top_message_id, topic.unread_messages_count, topic.unread_reactions_count);
        let read = (topic.last_read_inbox_message_id, topic.last_read_outbox_message_id);
        assert_eq!((counts, read), ((40, 3, 2), (30, 31)));
        assert_eq!((topic.unread_mark, topic.can_send_unpaid_messages), (true, true));
    }

    #[tokio::test]
    async fn a_topic_user_missing_from_the_reply_keeps_its_id() {
        let (client, _rx) = settings_client(vec![saved_dialogs(vec![monoforum_dialog(USER)], Vec::new())]);
        let topics = client.get_direct_messages_topics_by_id(get_channel_id(9), &[USER]).await.unwrap();
        assert_eq!(topics[0].topic.user, Some(User { id: USER, ..User::default() }));
    }

    #[tokio::test]
    async fn unchanged_direct_messages_topics_are_an_empty_list() {
        let not_modified = types::messages::SavedDialogsNotModified { count: 3 };
        let reply = Ok(base::messages::SavedDialogs::SavedDialogsNotModified(not_modified).to_bytes());
        let (client, _rx) = settings_client(vec![reply]);
        assert!(client.get_direct_messages_topics_by_id(get_channel_id(9), &[USER]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn chat_settings_of_a_chat_with_no_action_bar_are_the_default() {
        let settings = types::PeerSettings::deserialize_bare(&mut &[0u8; 4][..]).unwrap();
        let settings = base::PeerSettings::PeerSettings(Box::new(settings));
        let reply = types::messages::PeerSettings { settings, chats: Vec::new(), users: Vec::new() };
        let reply = Ok(base::messages::PeerSettings::PeerSettings(Box::new(reply)).to_bytes());
        let (client, mut rx) = settings_client(vec![reply]);
        assert_eq!(client.get_chat_settings(USER).await.unwrap(), ChatSettings::default());
        assert_eq!(rx.try_recv().unwrap(), messages::GetPeerSettings { peer: user_peer() }.to_bytes());
    }

    #[tokio::test]
    async fn top_chat_categories_set_their_flags() {
        use TopChatCategory as C;
        let (client, mut rx) = Client::recording();
        let cases = [
            (C::Users, 0x1),
            (C::Bots, 0x2),
            (C::InlineBots, 0x4),
            (C::Calls, 0x8),
            (C::ForwardChats, 0x30),
            (C::Groups, 0x400),
            (C::Channels, 0x8000),
            (C::WebAppBots, 0x1_0000),
            (C::GuestBots, 0x2_0000),
        ];
        for (category, flags) in cases {
            let sent = body(&mut rx, client.get_top_chats(category, 10)).await;
            assert_eq!(u32::from_le_bytes(sent[4..8].try_into().unwrap()), flags, "{category:?}");
        }
    }

    #[tokio::test]
    async fn top_chats_ask_once_and_refuse_a_zero_limit() {
        let (client, mut rx) = Client::recording();
        let refused = client.get_top_chats(TopChatCategory::Users, 0).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(rx.try_recv().is_err());

        let request = contacts::GetTopPeers {
            correspondents: true,
            bots_pm: false,
            bots_inline: false,
            phone_calls: false,
            forward_users: false,
            forward_chats: false,
            groups: false,
            channels: false,
            bots_app: false,
            bots_guestchat: false,
            offset: 0,
            limit: 10,
            hash: 0,
        };
        assert_eq!(body(&mut rx, client.get_top_chats(TopChatCategory::Users, 10)).await, request.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    fn top_category(category: base::TopPeerCategory, peers: Vec<(i64, f64)>) -> base::TopPeerCategoryPeers {
        let peers: Vec<_> = peers
            .into_iter()
            .map(|(id, rating)| base::TopPeer::TopPeer(Box::new(types::TopPeer { peer: get_peer(id), rating })))
            .collect();
        let category = types::TopPeerCategoryPeers { category, count: peers.len() as i32, peers };
        base::TopPeerCategoryPeers::TopPeerCategoryPeers(Box::new(category))
    }

    #[tokio::test]
    async fn forwarded_top_chats_merge_by_rating() {
        use base::TopPeerCategory as T;
        let categories = vec![
            top_category(T::TopPeerCategoryForwardUsers, vec![(USER, 1.0), (8, 3.0)]),
            top_category(T::TopPeerCategoryForwardChats, vec![(GROUP, 2.0)]),
        ];
        let top = types::contacts::TopPeers { categories, chats: Vec::new(), users: Vec::new() };
        let (client, _rx) = settings_client(vec![Ok(base::contacts::TopPeers::TopPeers(Box::new(top)).to_bytes())]);
        let chats = client.get_top_chats(TopChatCategory::ForwardChats, 10).await.unwrap();
        assert_eq!(chats.iter().map(|c| c.id).collect::<Vec<_>>(), [8, GROUP, USER]);
    }

    #[tokio::test]
    async fn disabled_or_unchanged_top_chats_are_empty() {
        // Disabled means the user switched suggestions off, so there are none.
        let replies = [base::contacts::TopPeers::TopPeersDisabled, base::contacts::TopPeers::TopPeersNotModified];
        let (client, _rx) = settings_client(replies.iter().map(|r| Ok(r.to_bytes())).collect());
        for _ in replies {
            assert!(client.get_top_chats(TopChatCategory::Users, 10).await.unwrap().is_empty());
        }
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
