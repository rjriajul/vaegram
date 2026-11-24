use std::sync::Arc;

use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::{Error, Result};
use crate::session::transfer::Report;
use crate::types::messages_and_media::sticker_source::Kind;
use crate::types::messages_and_media::{Source, media_setters};
use crate::types::{FileSource, InputMedia, Message, PeerRef, Sticker, StickerSet, StickerSetRef, StickerSource};

/// Request built by [`Client::send_sticker`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendSticker<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    // The set and sticker id to send, looked up when awaited; `media` holds a placeholder file until then.
    in_set: Option<(StickerSetRef, i64)>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    /// Sends a sticker: a file, or one picked from a set with [`StickerSource::from_set`].
    ///
    /// Awaiting the request uploads the file if needed and returns the sent [`crate::types::Message`].
    pub fn send_sticker<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        sticker: impl Into<StickerSource<'a>>,
    ) -> SendSticker<'a> {
        let sticker: StickerSource<'a> = sticker.into();
        let (file, in_set) = match sticker.0 {
            Kind::File(file) => (file, None),
            // Replaced by the sticker itself before sending; a URL sticker would be refused, never sent.
            Kind::InSet(set, id) => (FileSource::sourced(Source::Url(String::new()), None), Some((set, id))),
        };
        let media = InputMedia::sticker(file);
        SendSticker {
            client: self,
            chat_id: chat_id.into(),
            media,
            in_set,
            options: SendOptions::default(),
            progress: None,
        }
    }

    /// The sticker `id` of `set`, from the client's copy of the set if it has the sticker. Otherwise, or with
    /// `reload` (the copy's file reference expired), the set is read again: whole on `reload` or without a copy,
    /// else only if its hash changed.
    async fn sticker_in_set(&self, set: &StickerSetRef, id: i64, reload: bool) -> Result<Sticker> {
        let kept = if reload { None } else { self.0.caches.sticker_set(set) };
        if let Some(sticker) = kept.as_ref().and_then(|kept| kept.stickers.iter().find(|s| s.id == id)) {
            return Ok(sticker.clone());
        }
        let hash = kept.as_ref().map_or(0, |kept| kept.hash);
        let request = crate::raw::functions::messages::GetStickerSet { stickerset: set.input(), hash };
        let loaded = match StickerSet::parse_modified(self.invoke(&request).await?) {
            Some(loaded) => {
                let loaded = Arc::new(loaded);
                self.0.caches.store_sticker_set(set.clone(), loaded.clone());
                loaded
            }
            None => kept.ok_or(Error::Unexpected("Telegram returned stickerSetNotModified"))?,
        };
        let sticker = loaded.stickers.iter().find(|s| s.id == id);
        sticker.cloned().ok_or(Error::InvalidArgument("the sticker set has no sticker with this id"))
    }
}

impl SendSticker<'_> {
    async fn send(mut self) -> Result<Message> {
        // A sticker file goes in the `sendSticker` request; one picked from a set needs the set read first.
        #[cfg(feature = "bot-api")]
        if let (None, Some(bot)) = (&self.in_set, self.client.0.bot_api.as_deref()) {
            return bot.send_media(self.client, &self.chat_id, self.media, self.options).await;
        }
        let peer = self.client.resolve(&self.chat_id).await?.1;
        let Some((set, id)) = self.in_set.take() else {
            let business = self.options.business_connection_id.as_deref();
            let media = self.client.input_media(&peer, &self.media, self.progress.take(), business).await?;
            self.options.invert_media = self.media.show_caption_above_media;
            let (caption, parse_mode) = (self.media.caption, self.media.parse_mode);
            return self.client.send_media(self.chat_id, media, caption, parse_mode, self.options).await;
        };
        // A kept set's file references expire in time: on that error the set is read again, once.
        let mut reload = false;
        loop {
            let sticker = self.client.sticker_in_set(&set, id, reload).await?;
            self.media.file = FileSource::from(&sticker);
            let business = self.options.business_connection_id.as_deref();
            let media = self.client.input_media(&peer, &self.media, None, business).await?;
            let (caption, parse_mode) = (self.media.caption.clone(), self.media.parse_mode);
            let options = self.options.clone();
            match self.client.send_media(self.chat_id.clone(), media, caption, parse_mode, options).await {
                Err(e) if e.is_file_reference_error() && !reload => reload = true,
                result => return result,
            }
        }
    }
}

media_setters!(SendSticker.media: emoji mask_position progress);
send_builder!(SendSticker);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::StickerSource;
    use crate::types::messages_and_media::sticker_set::tests::set;

    fn cats() -> crate::Result<Vec<u8>> {
        Ok(set(false, false, None).to_bytes())
    }

    fn refused(name: &str) -> crate::Result<Vec<u8>> {
        Err(Error::Rpc(RpcError::parse(400, name)))
    }

    fn get_set(hash: i32) -> Vec<u8> {
        let name = types::InputStickerSetShortName { short_name: "cats_by_bot".into() };
        let stickerset = base::InputStickerSet::InputStickerSetShortName(Box::new(name));
        messages::GetStickerSet { stickerset, hash }.to_bytes()
    }

    /// Whether `body` is a `messages.sendMedia` of the set's sticker `id`.
    fn sends(body: &[u8], id: i64) -> bool {
        let document = types::InputDocument { id, access_hash: 2, file_reference: vec![3] };
        let media = base::InputMedia::InputMediaDocument(Box::new(types::InputMediaDocument {
            spoiler: false,
            id: base::InputDocument::InputDocument(Box::new(document)),
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
            query: None,
        }));
        let media = media.to_bytes();
        body[..4] == messages::SendMedia::CONSTRUCTOR_ID.to_le_bytes() && body.windows(media.len()).any(|w| w == media)
    }

    #[tokio::test]
    async fn sends_from_a_set_read_once() {
        let (client, mut sent) =
            Client::scripted_raw(vec![cats(), refused("CHAT_SEND_STICKERS_FORBIDDEN"), refused("X")]);
        let e = client.send_sticker(-5, StickerSource::from_set("cats_by_bot", 3)).await.err();
        assert!(e.is_some_and(|e| e.is("CHAT_SEND_STICKERS_FORBIDDEN")));
        assert_eq!(sent.try_recv().unwrap(), get_set(0));
        assert!(sends(&sent.try_recv().unwrap(), 3));

        // The kept set has sticker 1 too, so the second send reads nothing.
        assert!(client.send_sticker(-5, StickerSource::from_set("cats_by_bot", 1)).await.is_err());
        assert!(sends(&sent.try_recv().unwrap(), 1));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn reads_the_set_again_once_when_the_file_reference_expired() {
        let expired = || refused("FILE_REFERENCE_EXPIRED");
        let (client, mut sent) = Client::scripted_raw(vec![cats(), expired(), cats(), expired()]);
        let e = client.send_sticker(-5, StickerSource::from_set("cats_by_bot", 3)).await.err();
        assert!(e.is_some_and(|e| e.is_file_reference_error()));
        let bodies: Vec<_> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        assert_eq!(bodies.len(), 4, "read, send, read again, send again, then give up");
        assert_eq!((&bodies[0], &bodies[2]), (&get_set(0), &get_set(0)));
        assert!(sends(&bodies[1], 3) && sends(&bodies[3], 3));
    }

    #[tokio::test]
    async fn checks_the_set_hash_for_a_sticker_the_kept_set_lacks() {
        let unchanged = Ok(base::messages::StickerSet::StickerSetNotModified.to_bytes());
        let (client, mut sent) = Client::scripted_raw(vec![cats(), refused("X"), unchanged]);
        assert!(client.send_sticker(-5, StickerSource::from_set("cats_by_bot", 3)).await.is_err());
        let _ = (sent.try_recv().unwrap(), sent.try_recv().unwrap());

        let e = client.send_sticker(-5, StickerSource::from_set("cats_by_bot", 2)).await.err();
        assert!(matches!(e, Some(Error::InvalidArgument(_))), "{e:?}");
        assert_eq!(sent.try_recv().unwrap(), get_set(77));
        assert!(sent.try_recv().is_err(), "nothing is sent");
    }
}
