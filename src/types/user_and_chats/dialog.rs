use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, Message};
use crate::utils::get_peer_id;

#[derive(Debug, Clone)]
pub struct Dialog {
    pub chat: Chat,
    pub top_message: Option<Message>,
    pub unread_messages_count: i32,
    pub unread_mentions_count: i32,
    pub unread_reactions_count: i32,
    pub unread_mark: bool,
    pub is_pinned: bool,
    pub chat_list: Option<i32>,
}

impl Dialog {
    pub(crate) fn unpack(client: &Client, page: base::messages::Dialogs) -> (Vec<Dialog>, Option<i32>) {
        let (dialogs, mut messages, users, chats, count) = match page {
            base::messages::Dialogs::Dialogs(d) => (d.dialogs, d.messages, d.users, d.chats, None),
            base::messages::Dialogs::DialogsSlice(d) => (d.dialogs, d.messages, d.users, d.chats, Some(d.count)),
            base::messages::Dialogs::DialogsNotModified(d) => return (Vec::new(), Some(d.count)),
        };
        let entities = Entities::new(client, users, chats);
        let dialogs = dialogs
            .iter()
            .filter_map(|d| match d {
                // Folders and communities aren't chats.
                base::Dialog::Dialog(d) => Some(d),
                _ => None,
            })
            .map(|d| {
                let chat_id = get_peer_id(&d.peer);
                let at = messages.iter().position(|m| match m {
                    base::Message::Message(m) => m.id == d.top_message && get_peer_id(&m.peer_id) == chat_id,
                    base::Message::MessageService(m) => m.id == d.top_message && get_peer_id(&m.peer_id) == chat_id,
                    base::Message::MessageEmpty(_) => false,
                });
                let top_message = at.and_then(|at| Message::from_raw(client, messages.swap_remove(at), &entities));
                Dialog {
                    chat: Chat::parse(&d.peer, &entities, false),
                    top_message,
                    unread_messages_count: d.unread_count,
                    unread_mentions_count: d.unread_mentions_count,
                    unread_reactions_count: d.unread_reactions_count,
                    unread_mark: d.unread_mark,
                    is_pinned: d.pinned,
                    chat_list: d.folder_id,
                }
            })
            .collect();
        (dialogs, count)
    }
}

#[cfg(test)]
mod tests {
    use super::Dialog;
    use crate::Client;
    use crate::enums::ChatType;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::message::tests::service;

    fn notify_settings() -> base::PeerNotifySettings {
        base::PeerNotifySettings::PeerNotifySettings(Box::new(types::PeerNotifySettings {
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
        }))
    }

    #[test]
    fn unpacks_chats_and_skips_folders() {
        let peer = base::Peer::PeerChat(types::PeerChat { chat_id: 7 });
        let dialog = types::Dialog {
            pinned: true,
            unread_mark: true,
            view_forum_as_messages: false,
            peer: peer.clone(),
            top_message: 3,
            read_inbox_max_id: 0,
            read_outbox_max_id: 0,
            unread_count: 4,
            unread_mentions_count: 1,
            unread_reactions_count: 0,
            unread_poll_votes_count: 0,
            notify_settings: notify_settings(),
            pts: None,
            draft: None,
            folder_id: Some(1),
            ttl_period: None,
        };
        let folder = types::DialogFolder {
            pinned: true,
            folder: base::Folder::Folder(Box::new(types::Folder {
                autofill_new_broadcasts: false,
                autofill_public_groups: false,
                autofill_new_correspondents: false,
                id: 1,
                title: "Archive".into(),
                photo: None,
            })),
            peer,
            top_message: 3,
            unread_muted_peers_count: 0,
            unread_unmuted_peers_count: 0,
            unread_muted_messages_count: 0,
            unread_unmuted_messages_count: 0,
        };
        let chat = types::ChatForbidden { id: 7, title: "Old group".into() };
        let page = base::messages::Dialogs::DialogsSlice(Box::new(types::messages::DialogsSlice {
            count: 40,
            dialogs: vec![base::Dialog::DialogFolder(Box::new(folder)), base::Dialog::Dialog(Box::new(dialog))],
            messages: Vec::new(),
            chats: vec![base::Chat::ChatForbidden(Box::new(chat))],
            users: Vec::new(),
        }));

        let (dialogs, count) = Dialog::unpack(&Client::offline(), page);
        assert_eq!((dialogs.len(), count), (1, Some(40)));
        let d = &dialogs[0];
        assert_eq!((d.chat.id, d.chat.kind, d.chat.title.as_deref()), (-7, ChatType::Group, Some("Old group")));
        assert_eq!((d.unread_messages_count, d.unread_mentions_count), (4, 1));
        assert_eq!((d.is_pinned, d.unread_mark, d.top_message.is_none(), d.chat_list), (true, true, true, Some(1)));
    }

    #[test]
    fn a_service_message_is_a_top_message() {
        let peer = base::Peer::PeerChat(types::PeerChat { chat_id: 7 });
        let dialog = types::Dialog {
            pinned: false,
            unread_mark: false,
            view_forum_as_messages: false,
            peer: peer.clone(),
            top_message: 3,
            read_inbox_max_id: 0,
            read_outbox_max_id: 0,
            unread_count: 0,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            unread_poll_votes_count: 0,
            notify_settings: notify_settings(),
            pts: None,
            draft: None,
            folder_id: None,
            ttl_period: None,
        };
        let title = types::MessageActionChatEditTitle { title: "Renamed".into() };
        let renamed = types::MessageService {
            id: 3,
            peer_id: peer,
            ..service(base::MessageAction::MessageActionChatEditTitle(Box::new(title)))
        };
        let page = base::messages::Dialogs::Dialogs(Box::new(types::messages::Dialogs {
            dialogs: vec![base::Dialog::Dialog(Box::new(dialog))],
            messages: vec![base::Message::MessageService(Box::new(renamed))],
            chats: Vec::new(),
            users: Vec::new(),
        }));
        let (dialogs, _) = Dialog::unpack(&Client::offline(), page);
        assert_eq!(dialogs[0].top_message.as_ref().map(|m| m.id), Some(3));
    }
}
