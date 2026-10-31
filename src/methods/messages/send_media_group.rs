use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::session::transfer::try_join_all;
use crate::types::{InputMedia, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMediaGroup<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: Vec<InputMedia<'a>>,
    options: SendOptions,
}

impl Client {
    /// Sends 2–10 photos and videos, or only audio, or only documents, as one album.
    pub fn send_media_group<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        media: impl IntoIterator<Item = InputMedia<'a>>,
    ) -> SendMediaGroup<'a> {
        let media = media.into_iter().collect();
        SendMediaGroup { client: self, chat_id: chat_id.into(), media, options: SendOptions::default() }
    }
}

impl SendMediaGroup<'_> {
    async fn send(self) -> Result<Vec<Message>> {
        let (client, options) = (self.client, self.options);
        if options.suggested_post_parameters.is_some() {
            return Err(Error::InvalidArgument("an album can't be a suggested post"));
        }
        if !(2..=10).contains(&self.media.len()) {
            return Err(Error::InvalidArgument("an album holds 2 to 10 items"));
        }
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let invert_media = self.media.iter().any(|m| m.show_caption_above_media);
        let business = options.business_connection_id.as_deref();
        let items = self.media.into_iter().map(|item| single_media(client, &peer, item, business));
        let multi_media = try_join_all(items).await?;
        let request = functions::messages::SendMultiMedia {
            silent: options.disable_notification,
            background: false,
            clear_draft: false,
            noforwards: options.protect_content,
            update_stickersets_order: false,
            invert_media,
            allow_paid_floodskip: options.allow_paid_broadcast,
            reply_to: options.input_reply_to(client, chat_id, options.thread()).await?,
            peer,
            multi_media,
            schedule_date: options.schedule_date,
            send_as: None,
            quick_reply_shortcut: None,
            effect: options.message_effect_id,
            allow_paid_stars: options.paid_message_star_count,
        };
        let updates = client.invoke_business(business, request).await?.0;
        let mut messages: Vec<_> = Message::all_from_updates(client, updates).collect();
        if messages.is_empty() {
            return Err(Error::Unexpected("messages.sendMultiMedia returned no messages"));
        }
        messages.sort_unstable_by_key(|m| m.id);
        Ok(messages)
    }
}

async fn single_media(
    client: &Client,
    peer: &base::InputPeer,
    item: InputMedia<'_>,
    business: Option<&str>,
) -> Result<base::InputSingleMedia> {
    let media = client.upload_media(peer, client.input_media(peer, &item, None, business).await?, business).await?;
    let (message, entities) = client.write_text(item.caption, item.parse_mode).await?;
    let entities = (!entities.is_empty()).then_some(entities);
    let random_id = i64::from_le_bytes(random());
    let single = types::InputSingleMedia { media, random_id, message, entities };
    Ok(base::InputSingleMedia::InputSingleMedia(Box::new(single)))
}

impl SendMediaGroup<'_> {
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.options.business_connection_id = Some(id.into());
        self
    }
}

send_builder!(SendMediaGroup, Vec<Message>);

impl crate::methods::OnBehalf for SendMediaGroup<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.options.business_connection_id
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::raw::{Identifiable, functions};
    use crate::types::{FileSource, InputMedia};

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAQADAgADbQADNgQ";

    #[tokio::test]
    async fn a_business_album_goes_through_the_connection() {
        let (client, mut rx) = Client::recording();
        let photo = || InputMedia::photo(FileSource::file_id(PHOTO).unwrap());
        let album = client.send_media_group(-5, [photo(), photo()]).business_connection_id("conn");
        assert_eq!(
            sent(&mut rx, album).await,
            functions::InvokeWithBusinessConnection::<functions::messages::SendMultiMedia>::CONSTRUCTOR_ID
        );
    }
}
