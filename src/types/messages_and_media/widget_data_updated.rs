use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::Chat;

/// A change to a message widget's public or chat-specific data.
#[derive(Clone)]
pub struct WidgetDataUpdated {
    /// The chat containing the message.
    pub chat: Chat,
    /// Id of the message containing the widget.
    pub message_id: i32,
    /// The new public data, or `None` if it did not change.
    pub public_data: Option<String>,
    /// Hash of the new public data, or `None` if it did not change.
    pub public_data_hash: Option<i64>,
    /// The new chat-specific data, or `None` if it did not change.
    pub chat_data: Option<String>,
    client: Client,
}

impl WidgetDataUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateMessageWidgetData, entities: &Entities) -> Self {
        Self {
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            public_data: raw.public_data.clone(),
            public_data_hash: raw.public_data_hash,
            chat_data: raw.chat_data.clone(),
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl fmt::Debug for WidgetDataUpdated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetDataUpdated")
            .field("chat", &self.chat)
            .field("message_id", &self.message_id)
            .field("public_data", &self.public_data)
            .field("public_data_hash", &self.public_data_hash)
            .field("chat_data", &self.chat_data)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::WidgetDataUpdated;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::types;
    use crate::utils::get_peer;

    #[test]
    fn parses_changed_and_unchanged_widget_data() {
        let client = Client::offline();
        let entities = Entities::default();
        let mut raw = types::UpdateMessageWidgetData {
            peer: get_peer(7),
            msg_id: 42,
            public_data: Some("public".into()),
            public_data_hash: Some(123),
            chat_data: Some("chat".into()),
            pts: 1,
            pts_count: 1,
        };
        let event = WidgetDataUpdated::parse(&client, &raw, &entities);
        assert_eq!((event.chat.id, event.message_id), (7, 42));
        assert_eq!(event.public_data.as_deref(), Some("public"));
        assert_eq!(event.public_data_hash, Some(123));
        assert_eq!(event.chat_data.as_deref(), Some("chat"));

        raw.public_data = None;
        raw.public_data_hash = None;
        raw.chat_data = None;
        let event = WidgetDataUpdated::parse(&client, &raw, &entities);
        assert_eq!((event.public_data, event.public_data_hash, event.chat_data), (None, None, None));
    }
}
