use crate::errors::Result;
use crate::methods::{EditEphemeralMessage, EditInlineMedia, EditMessageMedia, into_future};
use crate::types::{
    CallbackOrigin, CallbackQuery, EditCallbackMessageText, InputMedia, MessageText, ReplyMarkup, Text,
};

impl CallbackQuery {
    /// Edits the caption of the message the button is on. Calls [`CallbackQuery::edit_message_text`], since
    /// one request edits text and captions alike.
    pub fn edit_message_caption(&self, caption: impl Into<Text>) -> EditCallbackMessageText<'_> {
        self.edit_message_text(MessageText::Text(caption.into()))
    }

    /// Replaces the media of the message the button is on.
    ///
    /// An inline message takes only a file already on Telegram, not a new upload.
    pub fn edit_message_media<'a>(&'a self, media: InputMedia<'a>) -> EditCallbackMessageMedia<'a> {
        let client = self.client();
        match &self.origin {
            CallbackOrigin::Message { chat, message_id, business_connection_id } => {
                let edit = client.edit_message_media(chat.id, *message_id, media);
                EditCallbackMessageMedia::Message(match business_connection_id {
                    Some(connection) => edit.business_connection_id(connection.clone()),
                    None => edit,
                })
            }
            CallbackOrigin::Inline(id) => EditCallbackMessageMedia::Inline(client.edit_inline_media(id.clone(), media)),
            CallbackOrigin::Ephemeral(message) => EditCallbackMessageMedia::Ephemeral(message.edit_media(media)),
        }
    }
}

/// A media edit of the message behind a [`CallbackQuery`]. Awaiting it sends the edit and returns `()`.
#[must_use = "a request does nothing until it is awaited"]
pub enum EditCallbackMessageMedia<'a> {
    /// Edit of a regular message.
    Message(EditMessageMedia<'a>),
    /// Edit of an inline message.
    Inline(EditInlineMedia<'a>),
    /// Edit of an ephemeral message.
    Ephemeral(EditEphemeralMessage<'a>),
}

impl EditCallbackMessageMedia<'_> {
    /// Replaces the message's keyboard.
    pub fn reply_markup(self, markup: impl Into<ReplyMarkup>) -> Self {
        match self {
            Self::Message(edit) => Self::Message(edit.reply_markup(markup)),
            Self::Inline(edit) => Self::Inline(edit.reply_markup(markup)),
            Self::Ephemeral(edit) => Self::Ephemeral(edit.reply_markup(markup)),
        }
    }

    async fn send(self) -> Result<()> {
        match self {
            Self::Message(edit) => edit.await.map(drop),
            Self::Inline(edit) => edit.await.map(drop),
            Self::Ephemeral(edit) => edit.await.map(drop),
        }
    }
}

into_future!(EditCallbackMessageMedia, ());

#[cfg(test)]
mod tests {
    use super::EditCallbackMessageMedia;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::dispatcher::listeners::tests::{inline_query, query};
    use crate::raw::{Serializable, base, types};
    use crate::types::messages_and_media::ephemeral_message::tests::{channel, ephemeral};
    use crate::types::{CallbackQuery, EditCallbackMessageText, FileSource, InputMedia, Message, Text};
    use crate::utils::get_peer;

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";

    fn photo() -> InputMedia<'static> {
        InputMedia::photo(FileSource::file_id(PHOTO).unwrap())
    }

    #[test]
    fn each_origin_edits_its_own_kind_of_message() {
        let client = Client::offline();
        let q = query(&client, 7, 7, 5);
        assert!(matches!(q.edit_message_media(photo()), EditCallbackMessageMedia::Message(_)));
        assert!(matches!(q.edit_message_caption("new"), EditCallbackMessageText::Message(_)));

        let q = inline_query(&client, 7, 1);
        assert!(matches!(q.edit_message_media(photo()), EditCallbackMessageMedia::Inline(_)));
        assert!(matches!(q.edit_message_caption("new"), EditCallbackMessageText::Inline(_)));

        let update = types::UpdateEphemeralBotCallbackQuery {
            query_id: 9,
            user_id: 1000,
            peer: None,
            msg_id: 7,
            data: b"tap".to_vec(),
            chat_instance: None,
            message: ephemeral(true, Some(channel())),
        };
        let q = CallbackQuery::parse_ephemeral(&client, update, &Entities::default());
        assert!(matches!(q.edit_message_media(photo()), EditCallbackMessageMedia::Ephemeral(_)));
        assert!(matches!(q.edit_message_caption("new"), EditCallbackMessageText::Ephemeral(_)));
    }

    #[tokio::test]
    async fn business_media_edits_go_through_the_connection() {
        let update = types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(Message::bare(5, get_peer(7), 0))),
            reply_to_message: None,
            chat_instance: 3,
            data: Some(b"buy".to_vec()),
        };
        let (client, mut sent) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, peer);
        let q = CallbackQuery::parse_business(&client, update, &Entities::default());
        assert!(q.edit_message_media(photo()).await.is_err());
        assert_eq!(sent.try_recv().expect("no request sent")[..4], 0xdd289f8e_u32.to_le_bytes());
    }

    #[tokio::test]
    async fn a_built_caption_keeps_its_entities() {
        let (client, mut sent) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, peer);
        let _ = query(&client, 7, 7, 5).edit_message_caption(Text::new().bold("new")).await;
        let body = sent.try_recv().expect("no request sent");
        let bold = vec![base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 3 })];
        let has = |bytes: &[u8]| body.windows(bytes.len()).any(|w| w == bytes);
        assert!(has(&String::from("new").to_bytes()) && has(&bold.to_bytes()));
    }
}
