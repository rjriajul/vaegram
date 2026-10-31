use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::base;
use crate::types::messages_and_media::{Source, media_setters};
use crate::types::{FileSource, InputMedia, Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendCachedMedia<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    file_id: String,
    caption: Text,
    parse_mode: Option<ParseMode>,
    spoiler: bool,
    show_caption_above_media: bool,
    options: SendOptions,
}

impl Client {
    /// Sends a file already on Telegram by its file id, as a photo or a document depending on what the id names.
    pub fn send_cached_media(&self, chat_id: impl Into<PeerRef>, file_id: impl Into<String>) -> SendCachedMedia<'_> {
        SendCachedMedia {
            client: self,
            chat_id: chat_id.into(),
            file_id: file_id.into(),
            caption: Text::new(),
            parse_mode: None,
            spoiler: false,
            show_caption_above_media: false,
            options: SendOptions::default(),
        }
    }
}

impl SendCachedMedia<'_> {
    async fn send(mut self) -> Result<Message> {
        let source = FileSource::file_id(&self.file_id)?;
        let photo = matches!(source.0, Source::Stored(base::InputMedia::InputMediaPhoto(_)));
        let media = if photo { InputMedia::photo(source) } else { InputMedia::document(source) };
        let media =
            InputMedia { spoiler: self.spoiler, show_caption_above_media: self.show_caption_above_media, ..media };
        let peer = self.client.resolve(&self.chat_id).await?.1;
        let business = self.options.business_connection_id.as_deref();
        let media = self.client.input_media(&peer, &media, None, business).await?;
        self.options.invert_media = self.show_caption_above_media;
        self.client.send_media(self.chat_id, media, self.caption, self.parse_mode, self.options).await
    }
}

media_setters!(SendCachedMedia: caption has_spoiler show_caption_above_media);
send_builder!(SendCachedMedia);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::errors::Error;
    use crate::raw::functions::{InvokeWithBusinessConnection, messages};
    use crate::raw::{Identifiable, types};

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";
    const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";

    fn contains(body: &[u8], id: u32) -> bool {
        body.windows(4).any(|w| w == id.to_le_bytes())
    }

    #[tokio::test]
    async fn a_cached_photo_goes_as_a_photo() {
        let (client, mut rx) = Client::recording();
        let _ = client.send_cached_media(-5, PHOTO).await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(body[..4], messages::SendMedia::CONSTRUCTOR_ID.to_le_bytes());
        assert!(contains(&body, types::InputMediaPhoto::CONSTRUCTOR_ID));
        assert!(!contains(&body, types::InputMediaDocument::CONSTRUCTOR_ID));
    }

    #[tokio::test]
    async fn a_cached_document_goes_as_a_document() {
        let (client, mut rx) = Client::recording();
        let _ = client.send_cached_media(-5, DOCUMENT).await;
        let body = rx.try_recv().expect("no request sent");
        assert!(contains(&body, types::InputMediaDocument::CONSTRUCTOR_ID));
        assert!(!contains(&body, types::InputMediaPhoto::CONSTRUCTOR_ID));
    }

    #[tokio::test]
    async fn caption_spoiler_and_above_media_are_sent() {
        let (client, mut rx) = Client::recording();
        let _ = client.send_cached_media(-5, PHOTO).caption("again").has_spoiler().show_caption_above_media().await;
        let body = rx.try_recv().expect("no request sent");
        let word = |at: usize| u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
        // messages.sendMedia's invert_media is flags.16; inputMediaPhoto's spoiler is flags.1.
        assert_eq!((word(0), word(4) & (1 << 16)), (messages::SendMedia::CONSTRUCTOR_ID, 1 << 16));
        let at = body.windows(4).position(|w| w == types::InputMediaPhoto::CONSTRUCTOR_ID.to_le_bytes()).unwrap();
        assert_eq!(word(at + 4) & (1 << 1), 1 << 1);
        assert!(body.windows(5).any(|w| w == b"again"));
    }

    #[tokio::test]
    async fn a_bad_file_id_fails_without_a_request() {
        let (client, mut rx) = Client::recording();
        let result = client.send_cached_media(-5, "AQADAQ*bad").await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn cached_media_goes_through_the_business_connection() {
        let (client, mut rx) = Client::recording();
        let media = client.send_cached_media(-5, DOCUMENT).business_connection_id("c1");
        assert_eq!(sent(&mut rx, media).await, InvokeWithBusinessConnection::<messages::SendMedia>::CONSTRUCTOR_ID);
    }
}
