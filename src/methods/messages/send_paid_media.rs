use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::{MediaKind, ParseMode};
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::session::transfer::try_join_all;
use crate::types::{InputMedia, Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendPaidMedia<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    star_count: i64,
    media: Vec<InputMedia<'a>>,
    payload: Option<String>,
    caption: Text,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    /// 1 to 10 photos, videos or live photos behind one caption; item captions are ignored.
    pub fn send_paid_media<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        star_count: i64,
        media: impl IntoIterator<Item = InputMedia<'a>>,
    ) -> SendPaidMedia<'a> {
        SendPaidMedia {
            client: self,
            chat_id: chat_id.into(),
            star_count,
            media: media.into_iter().collect(),
            payload: None,
            caption: Text::new(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }
}

impl SendPaidMedia<'_> {
    pub fn payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub fn caption(mut self, caption: impl Into<Text>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn show_caption_above_media(mut self) -> Self {
        self.options.invert_media = true;
        self
    }

    async fn send(self) -> Result<Message> {
        let paid = |m: &InputMedia| matches!(m.kind, MediaKind::Photo | MediaKind::Video | MediaKind::LivePhoto);
        if !(1..=10).contains(&self.media.len()) || !self.media.iter().all(paid) {
            return Err(Error::InvalidArgument("paid media takes 1 to 10 photos, videos and live photos"));
        }
        let client = self.client;
        let peer = client.resolve(&self.chat_id).await?.1;
        // Paid media only takes stored files, like albums, so each is stored first; they upload side by side.
        let business = self.options.business_connection_id.as_deref();
        let extended_media = try_join_all(self.media.iter().map(|item| async {
            let media = client.input_media(&peer, item, None, business).await?;
            client.upload_media(&peer, media, business).await
        }))
        .await?;
        let (stars_amount, payload) = (self.star_count, self.payload);
        let paid = types::InputMediaPaidMedia { stars_amount, extended_media, payload };
        let media = base::InputMedia::InputMediaPaidMedia(Box::new(paid));
        client.send_media(self.chat_id, media, self.caption, self.parse_mode, self.options).await
    }
}

caption_setters!(SendPaidMedia);
send_builder!(SendPaidMedia);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::types::InputMedia;

    #[tokio::test]
    async fn rejects_other_kinds_and_other_than_1_to_10_items_without_a_request() {
        let (client, mut sent) = Client::recording();
        let document = client.send_paid_media(-5, 5, [InputMedia::document("a.pdf")]).await.err();
        assert!(matches!(document, Some(Error::InvalidArgument(_))), "{document:?}");
        let eleven = client.send_paid_media(-5, 5, (0..11).map(|_| InputMedia::photo("a.jpg"))).await.err();
        assert!(matches!(eleven, Some(Error::InvalidArgument(_))), "{eleven:?}");
        let none = client.send_paid_media(-5, 5, []).await.err();
        assert!(matches!(none, Some(Error::InvalidArgument(_))), "{none:?}");
        assert!(sent.try_recv().is_err());
    }
}
