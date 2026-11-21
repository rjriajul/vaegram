use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::{Media, MessageEntity};

/// A notice from Telegram itself, such as a warning that the account was limited or frozen. Official apps show it
/// in the chat with the Telegram service account, or as a popup.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ServiceNotification {
    /// Telegram's name for the kind of notice, such as `AUTH_KEY_DROP_DUPLICATE`. Notices of the same kind replace
    /// each other.
    pub kind: String,
    /// The notice's text, without formatting.
    pub text: String,
    /// Formatting of the text. `None` if it has none.
    pub entities: Option<Vec<MessageEntity>>,
    /// Media that came with the notice. `None` if it has none.
    pub media: Option<Media>,
    /// `true` if the notice should be shown as a popup rather than stored in the service chat.
    pub is_popup: bool,
    /// `true` if the media should be shown above the text.
    pub show_caption_above_media: bool,
    /// Unix time to show the notice in the service chat with. `None` for a popup that isn't stored there.
    pub date: Option<i32>,
    client: Client,
}

impl ServiceNotification {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateServiceNotification, entities: &Entities) -> Self {
        ServiceNotification {
            kind: raw.r#type.clone(),
            text: raw.message.clone(),
            entities: MessageEntity::parse_all(&raw.entities, entities),
            media: Media::parse(client, &raw.media, entities),
            is_popup: raw.popup,
            show_caption_above_media: raw.invert_media,
            date: raw.inbox_date,
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
