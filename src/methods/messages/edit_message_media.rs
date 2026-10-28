use super::edit_message_text::edit_request;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputMedia, Message, PeerRef, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessageMedia<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    media: InputMedia<'a>,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn edit_message_media<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        media: InputMedia<'a>,
    ) -> EditMessageMedia<'a> {
        EditMessageMedia {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            media,
            reply_markup: None,
            business_connection_id: None,
        }
    }
}

impl EditMessageMedia<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let business = self.business_connection_id.as_deref();
        let media = client.input_media(&peer, &self.media, None, business).await?;
        let (message, entities) = client.write_text(self.media.caption, self.media.parse_mode).await?;
        let request = functions::messages::EditMessage {
            invert_media: self.media.show_caption_above_media,
            message: Some(message),
            media: Some(media),
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            entities: (!entities.is_empty()).then_some(entities),
            ..edit_request(peer, self.message_id)
        };
        let (updates, request) = client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Message::from_updates(client, updates, chat_id, (request.message.unwrap_or_default(), request.entities))
    }
}

into_future!(EditMessageMedia, Message);

impl crate::methods::OnBehalf for EditMessageMedia<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::messages_and_media::Source;
    use crate::types::{FileSource, InlineMessageId, InputMedia};

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
    const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";

    fn stored(file_id: &str) -> base::InputMedia {
        let Source::Stored(media) = FileSource::file_id(file_id).unwrap().0 else { panic!("not a stored file") };
        media
    }

    #[tokio::test]
    async fn stored_files_carry_the_new_flags() {
        let (client, mut sent) = Client::recording();
        let video = InputMedia::video(FileSource::file_id(DOCUMENT).unwrap())
            .cover(FileSource::file_id(PHOTO).unwrap())
            .start_timestamp(3)
            .has_spoiler()
            .show_caption_above_media()
            .caption("<b>new</b>")
            .html();
        let _ = client.edit_message_media(-5, 7, video).await;

        let (base::InputMedia::InputMediaDocument(mut document), base::InputMedia::InputMediaPhoto(cover)) =
            (stored(DOCUMENT), stored(PHOTO))
        else {
            panic!("file ids decode to a document and a photo")
        };
        (document.spoiler, document.video_cover, document.video_timestamp) = (true, Some(cover.id), Some(3));
        let bold = types::MessageEntityBold { offset: 0, length: 3 };
        let request = messages::EditMessage {
            no_webpage: false,
            invert_media: true,
            peer: base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: 5 })),
            id: 7,
            message: Some("new".into()),
            media: Some(base::InputMedia::InputMediaDocument(document)),
            reply_markup: None,
            entities: Some(vec![base::MessageEntity::MessageEntityBold(Box::new(bold))]),
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
            rich_message: None,
        };
        assert_eq!(sent.try_recv().expect("no request sent"), request.to_bytes());
    }

    #[tokio::test]
    async fn bad_media_fails_without_a_request() {
        let (client, mut sent) = Client::recording();
        let id = types::InputBotInlineMessageID { dc_id: 2, id: 1, access_hash: 1 };
        let id = || InlineMessageId(base::InputBotInlineMessageID::InputBotInlineMessageID(Box::new(id.clone())));
        let upload = client.edit_inline_media(id(), InputMedia::photo("cat.jpg")).await;
        let cover = InputMedia::video(FileSource::file_id(DOCUMENT).unwrap()).cover("cover.jpg");
        let cover = client.edit_inline_media(id(), cover).await;
        let photo_as_video = client.send_video(-5, FileSource::file_id(PHOTO).unwrap()).await;
        let document_as_photo =
            client.edit_message_media(-5, 7, InputMedia::photo(FileSource::file_id(DOCUMENT).unwrap()));
        let document_as_photo = document_as_photo.await;
        let lone = client.send_media_group(-5, [InputMedia::photo(FileSource::file_id(PHOTO).unwrap())]).await;
        let results = [
            upload.map(|_| ()),
            cover.map(|_| ()),
            photo_as_video.map(|_| ()),
            document_as_photo.map(|_| ()),
            lone.map(|_| ()),
        ];
        for result in results {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(sent.try_recv().is_err());
    }
}
