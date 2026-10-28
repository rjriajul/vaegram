use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::messages_and_media::{StickerFile, input_document};
use crate::types::{Document, InputSticker, MaskPosition, PeerRef, Upload};

impl Client {
    /// Uploads a sticker file ahead of time for [`InputSticker`]. Its extension sets the format, as listed there.
    pub async fn upload_sticker_file(
        &self,
        user_id: impl Into<PeerRef>,
        sticker: impl Into<Upload<'_>>,
    ) -> Result<Document> {
        self.upload_sticker(&user_id.into(), sticker.into()).await
    }

    pub(crate) async fn upload_sticker(&self, user: &PeerRef, file: Upload<'_>) -> Result<Document> {
        let peer = self.resolve(user).await?.1;
        let sticker = types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        };
        let file_name = types::DocumentAttributeFilename { file_name: file.name().into() };
        let media = types::InputMediaUploadedDocument {
            nosound_video: false,
            force_file: false,
            spoiler: false,
            file: self.save_file(file).await?,
            thumb: None,
            mime_type: mime_type(file.name()).into(),
            attributes: vec![
                base::DocumentAttribute::DocumentAttributeSticker(Box::new(sticker)),
                base::DocumentAttribute::DocumentAttributeFilename(Box::new(file_name)),
            ],
            stickers: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
        };
        let media = base::InputMedia::InputMediaUploadedDocument(Box::new(media));
        let request = functions::messages::UploadMedia { business_connection_id: None, peer, media };
        let document = match self.invoke(&request).await? {
            base::MessageMedia::MessageMediaDocument(m) => m.document.as_ref().and_then(Document::parse),
            _ => None,
        };
        document.ok_or(Error::Unexpected("messages.uploadMedia returned no document"))
    }

    pub(crate) async fn sticker_document(&self, user: &PeerRef, file: StickerFile<'_>) -> Result<base::InputDocument> {
        match file {
            StickerFile::Upload(file) => Ok(input_document(&self.upload_sticker(user, file).await?.location)),
            StickerFile::Stored(location) => match input_document(&location) {
                base::InputDocument::InputDocumentEmpty => Err(Error::InvalidArgument("a sticker must be a document")),
                document => Ok(document),
            },
        }
    }

    pub(crate) async fn input_sticker(
        &self,
        user: &PeerRef,
        sticker: InputSticker<'_>,
    ) -> Result<base::InputStickerSetItem> {
        Ok(base::InputStickerSetItem::InputStickerSetItem(Box::new(types::InputStickerSetItem {
            document: self.sticker_document(user, sticker.file).await?,
            emoji: sticker.emoji,
            mask_coords: sticker.mask_position.as_ref().map(MaskPosition::write),
            keywords: sticker.keywords,
        })))
    }
}

// Telegram tells static, animated and video stickers apart by MIME type.
fn mime_type(name: &str) -> &'static str {
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    [("tgs", "application/x-tgsticker"), ("webm", "video/webm"), ("png", "image/png")]
        .into_iter()
        .find(|(e, _)| e.eq_ignore_ascii_case(extension))
        .map_or("image/webp", |(_, mime)| mime)
}

#[cfg(test)]
mod tests {
    use super::mime_type;

    #[test]
    fn picks_the_sticker_format_from_the_extension() {
        let names = ["cat.TGS", "cat.webm", "cat.png", "cat.webp", "cat"];
        let mimes = ["application/x-tgsticker", "video/webm", "image/png", "image/webp", "image/webp"];
        assert_eq!(names.map(mime_type), mimes);
    }
}
