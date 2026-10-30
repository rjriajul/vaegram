use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, Message};
use crate::utils::get_peer_id;

#[derive(Debug, Clone)]
pub struct SavedDialog {
    pub chat: Chat,
    pub top_message: Option<Message>,
    pub is_pinned: bool,
}

impl SavedDialog {
    pub(crate) fn unpack(
        client: &Client,
        page: base::messages::SavedDialogs,
    ) -> (Vec<SavedDialog>, usize, Option<(i32, i32, i64)>) {
        let (dialogs, mut messages, users, chats) = match page {
            base::messages::SavedDialogs::SavedDialogs(d) => (d.dialogs, d.messages, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsSlice(d) => (d.dialogs, d.messages, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsNotModified(_) => return (Vec::new(), 0, None),
        };
        let next = dialogs.last().and_then(|d| {
            let (peer, top) = match d {
                base::SavedDialog::SavedDialog(d) => (&d.peer, d.top_message),
                base::SavedDialog::MonoForumDialog(d) => (&d.peer, d.top_message),
            };
            let date = messages.iter().find_map(|m| match m {
                base::Message::Message(m) if m.id == top => Some(m.date),
                base::Message::MessageService(m) if m.id == top => Some(m.date),
                _ => None,
            })?;
            Some((date, top, get_peer_id(peer)))
        });
        let count = dialogs.len();
        let entities = Entities::new(client, users, chats);
        let dialogs = dialogs
            .into_iter()
            .filter_map(|d| match d {
                base::SavedDialog::SavedDialog(d) => Some(d),
                base::SavedDialog::MonoForumDialog(_) => None,
            })
            .map(|d| {
                // Everything in Saved Messages lives in your own chat, so the id alone is unique.
                let top_message = messages
                    .iter()
                    .position(|m| match m {
                        base::Message::Message(m) => m.id == d.top_message,
                        base::Message::MessageService(m) => m.id == d.top_message,
                        base::Message::MessageEmpty(_) => false,
                    })
                    .and_then(|i| Message::from_raw(client, messages.swap_remove(i), &entities));
                SavedDialog { chat: Chat::parse(&d.peer, &entities, false), top_message, is_pinned: d.pinned }
            })
            .collect();
        (dialogs, count, next)
    }
}
