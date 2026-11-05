use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{Chat, DirectMessagesChatTopic, Message};
use crate::utils::get_peer_id;

/// A chat that has messages saved in Saved Messages.
#[derive(Debug, Clone)]
pub struct SavedDialog {
    /// The chat the saved messages came from.
    pub chat: Chat,
    /// The latest saved message from the chat. `None` if the page did not include it.
    pub top_message: Option<Message>,
    /// The dialog is pinned in Saved Messages.
    pub is_pinned: bool,
}

// Where the next page starts: the last dialog's top message, when the page carries it.
fn next_cursor(dialogs: &[base::SavedDialog], messages: &[base::Message]) -> Option<(i32, i32, i64)> {
    let (peer, top) = match dialogs.last()? {
        base::SavedDialog::SavedDialog(d) => (&d.peer, d.top_message),
        base::SavedDialog::MonoForumDialog(d) => (&d.peer, d.top_message),
    };
    let date = messages.iter().find_map(|m| match m {
        base::Message::Message(m) if m.id == top => Some(m.date),
        base::Message::MessageService(m) if m.id == top => Some(m.date),
        _ => None,
    })?;
    Some((date, top, get_peer_id(peer)))
}

impl SavedDialog {
    /// Like `unpack`, for the topics of a direct messages chat: the topics, how many entries the page held,
    /// and the cursor for the next page.
    pub(crate) fn unpack_topics(
        client: &Client,
        page: base::messages::SavedDialogs,
    ) -> (Vec<DirectMessagesChatTopic>, usize, Option<(i32, i32, i64)>) {
        let (dialogs, messages, users, chats) = match page {
            base::messages::SavedDialogs::SavedDialogs(d) => (d.dialogs, d.messages, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsSlice(d) => (d.dialogs, d.messages, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsNotModified(_) => return (Vec::new(), 0, None),
        };
        let next = next_cursor(&dialogs, &messages);
        let count = dialogs.len();
        let entities = Entities::new(client, users, chats);
        let topics = dialogs
            .iter()
            .filter_map(|d| match d {
                base::SavedDialog::MonoForumDialog(d) => Some(DirectMessagesChatTopic::parse(d, &entities)),
                base::SavedDialog::SavedDialog(_) => None,
            })
            .collect();
        (topics, count, next)
    }

    pub(crate) fn unpack(
        client: &Client,
        page: base::messages::SavedDialogs,
    ) -> (Vec<SavedDialog>, usize, Option<(i32, i32, i64)>) {
        let (dialogs, mut messages, users, chats) = match page {
            base::messages::SavedDialogs::SavedDialogs(d) => (d.dialogs, d.messages, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsSlice(d) => (d.dialogs, d.messages, d.users, d.chats),
            base::messages::SavedDialogs::SavedDialogsNotModified(_) => return (Vec::new(), 0, None),
        };
        let next = next_cursor(&dialogs, &messages);
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
