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
mod delete_forum_topic;
mod delete_supergroup;
mod edit_forum_topic;
mod edit_general_forum_topic;
mod get_chat;
mod get_chat_administrators;
mod get_chat_member;
mod get_chat_members;
mod get_chat_members_count;
mod get_chat_notifications;
mod get_dialogs;
mod get_dialogs_count;
mod get_forum_topic_icon_stickers;
mod get_send_as_chats;
mod hide_general_forum_topic;
mod join_chat;
mod leave_chat;
mod mark_chat_unread;
mod mute_chat;
mod pin_chat;
mod promote_chat_member;
mod reopen_forum_topic;
mod reopen_general_forum_topic;
mod restrict_chat_member;
mod set_chat_administrator_custom_title;
mod set_chat_description;
mod set_chat_member_tag;
mod set_chat_permissions;
mod set_chat_photo;
mod set_chat_protected_content;
mod set_chat_sticker_set;
mod set_chat_title;
mod set_chat_username;
mod set_send_as_chat;
mod set_slow_mode;
mod toggle_forum_topics;
mod toggle_join_request;
mod toggle_join_to_send;
mod toggle_signatures;
mod transfer_chat_ownership;
mod unarchive_chats;
mod unban_chat_member;
mod unban_chat_sender_chat;
mod unhide_general_forum_topic;
mod unmute_chat;
mod unpin_all_forum_topic_messages;
mod unpin_all_general_forum_topic_messages;
mod unpin_chat;
mod update_chat_notifications;

pub use ban_chat_member::BanChatMember;
pub use create_forum_topic::CreateForumTopic;
pub use delete_chat_history::DeleteChatHistory;
pub use edit_forum_topic::EditForumTopic;
pub use get_chat_administrators::GetChatAdministrators;
pub use get_chat_members::GetChatMembers;
pub use get_dialogs::GetDialogs;
pub use restrict_chat_member::RestrictChatMember;
pub use set_chat_photo::SetChatPhoto;
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

        assert_eq!(sent(rx, client.add_chat_members(GROUP, [7], 100)).await, messages::AddChatUser::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, client.add_chat_members(channel, [7], 100)).await,
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
