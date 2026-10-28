use crate::Client;
use crate::enums::MediaKind;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::session::transfer::Report;
use crate::types::messages_and_media::{Source, input_document_of, input_photo_of};
use crate::types::{FileSource, InputMedia, Upload};

impl Client {
    pub(crate) async fn input_media(
        &self,
        peer: &base::InputPeer,
        media: &InputMedia<'_>,
        progress: Option<Report<'_>>,
        business_connection_id: Option<&str>,
    ) -> Result<base::InputMedia> {
        let photo = matches!(media.kind, MediaKind::Photo | MediaKind::LivePhoto);
        match &media.file.0 {
            Source::Stored(base::InputMedia::InputMediaPhoto(_)) if !photo => {
                return Err(Error::InvalidArgument("a stored photo can only be sent as a photo"));
            }
            Source::Stored(base::InputMedia::InputMediaDocument(_)) if photo => {
                return Err(Error::InvalidArgument("a stored document can't be sent as a photo"));
            }
            // The external media types carry no attributes, so only kinds that need none can come from a URL.
            Source::Url(_) if !matches!(media.kind, MediaKind::Photo | MediaKind::Document) => {
                return Err(Error::InvalidArgument("only photos and documents can be sent by URL; upload the file"));
            }
            Source::Url(_) if media.thumb.is_some() || media.force_file => {
                return Err(Error::InvalidArgument("a document sent by URL takes no thumbnail or force_file"));
            }
            _ => {}
        }
        let live_photo = media.kind == MediaKind::LivePhoto;
        let video = match &media.video {
            Some(video) => Some(self.live_video(peer, video, media, business_connection_id).await?),
            None => None,
        };
        let video_cover = self.cover(peer, media, business_connection_id).await?;
        let video_timestamp = media.start_timestamp;
        Ok(match &media.file.0 {
            Source::Stored(stored) => match stored.clone() {
                base::InputMedia::InputMediaPhoto(mut photo) => {
                    (photo.spoiler, photo.live_photo, photo.video) = (media.spoiler, live_photo, video);
                    base::InputMedia::InputMediaPhoto(photo)
                }
                base::InputMedia::InputMediaDocument(mut document) => {
                    document.spoiler = media.spoiler;
                    (document.video_cover, document.video_timestamp) = (video_cover, video_timestamp);
                    base::InputMedia::InputMediaDocument(document)
                }
                other => other,
            },
            Source::Url(url) if photo => {
                let external =
                    types::InputMediaPhotoExternal { spoiler: media.spoiler, url: url.clone(), ttl_seconds: None };
                base::InputMedia::InputMediaPhotoExternal(Box::new(external))
            }
            Source::Url(url) => {
                base::InputMedia::InputMediaDocumentExternal(Box::new(types::InputMediaDocumentExternal {
                    spoiler: media.spoiler,
                    url: url.clone(),
                    ttl_seconds: None,
                    video_cover,
                    video_timestamp,
                }))
            }
            Source::Upload(upload) if photo => {
                base::InputMedia::InputMediaUploadedPhoto(Box::new(types::InputMediaUploadedPhoto {
                    spoiler: media.spoiler,
                    live_photo,
                    file: self.save_file(*upload).progress(progress).await?,
                    stickers: None,
                    ttl_seconds: None,
                    video,
                }))
            }
            Source::Upload(upload) => {
                let thumb = async {
                    match media.thumb {
                        Some(thumb) => self.save_file(thumb).await.map(Some),
                        None => Ok(None),
                    }
                };
                let (file, thumb) = tokio::try_join!(self.save_file(*upload).progress(progress).into_future(), thumb)?;
                let mut document = uploaded_document(media.kind, media, *upload, file);
                (document.thumb, document.video_cover, document.video_timestamp) =
                    (thumb, video_cover, video_timestamp);
                base::InputMedia::InputMediaUploadedDocument(Box::new(document))
            }
        })
    }

    /// Business uploads go through the connection: the bot itself isn't in the customer's chat.
    pub(crate) async fn upload_media(
        &self,
        peer: &base::InputPeer,
        media: base::InputMedia,
        business_connection_id: Option<&str>,
    ) -> Result<base::InputMedia> {
        // The answer doesn't echo the per-message flags, so they are put back on the stored reference.
        let (spoiler, live, cover, timestamp) = match &media {
            base::InputMedia::InputMediaUploadedPhoto(m) => {
                (m.spoiler, Some((m.live_photo, m.video.clone())), None, None)
            }
            base::InputMedia::InputMediaUploadedDocument(m) => {
                (m.spoiler, None, m.video_cover.clone(), m.video_timestamp)
            }
            base::InputMedia::InputMediaPhotoExternal(m) => (m.spoiler, None, None, None),
            base::InputMedia::InputMediaDocumentExternal(m) => {
                (m.spoiler, None, m.video_cover.clone(), m.video_timestamp)
            }
            _ => return Ok(media),
        };
        let business_connection_id = business_connection_id.map(Into::into);
        let request = functions::messages::UploadMedia { business_connection_id, peer: peer.clone(), media };
        let mut stored = stored_media(self.invoke(&request).await?)
            .ok_or(Error::Unexpected("messages.uploadMedia returned no photo or document"))?;
        match &mut stored {
            base::InputMedia::InputMediaPhoto(p) => {
                p.spoiler = spoiler;
                if let Some((live_photo, video)) = live {
                    (p.live_photo, p.video) = (live_photo, video.or(p.video.take()));
                }
            }
            base::InputMedia::InputMediaDocument(d) => {
                d.spoiler = spoiler;
                (d.video_cover, d.video_timestamp) = (cover.or(d.video_cover.take()), timestamp.or(d.video_timestamp));
            }
            _ => {}
        }
        Ok(stored)
    }

    async fn live_video(
        &self,
        peer: &base::InputPeer,
        video: &FileSource<'_>,
        media: &InputMedia<'_>,
        business_connection_id: Option<&str>,
    ) -> Result<base::InputDocument> {
        let upload = match &video.0 {
            Source::Stored(base::InputMedia::InputMediaDocument(d)) => return Ok(d.id.clone()),
            Source::Stored(_) => return Err(Error::InvalidArgument("a live photo's video must be a video")),
            Source::Url(_) => return Err(Error::InvalidArgument("a live photo's video can't be sent by URL")),
            Source::Upload(upload) => *upload,
        };
        let mut uploaded = uploaded_document(MediaKind::Video, media, upload, self.save_file(upload).await?);
        uploaded.spoiler = false;
        let uploaded = base::InputMedia::InputMediaUploadedDocument(Box::new(uploaded));
        match self.upload_media(peer, uploaded, business_connection_id).await? {
            base::InputMedia::InputMediaDocument(d) => Ok(d.id),
            _ => Err(Error::Unexpected("messages.uploadMedia returned no document for the video")),
        }
    }

    async fn cover(
        &self,
        peer: &base::InputPeer,
        media: &InputMedia<'_>,
        business_connection_id: Option<&str>,
    ) -> Result<Option<base::InputPhoto>> {
        let photo = match media.cover.as_ref().map(|c| &c.0) {
            None => return Ok(None),
            Some(Source::Stored(base::InputMedia::InputMediaPhoto(p))) => return Ok(Some(p.id.clone())),
            Some(Source::Stored(_)) => return Err(Error::InvalidArgument("a video cover must be a photo")),
            Some(Source::Url(url)) => {
                let url = url.clone();
                base::InputMedia::InputMediaPhotoExternal(Box::new(types::InputMediaPhotoExternal {
                    spoiler: false,
                    url,
                    ttl_seconds: None,
                }))
            }
            Some(Source::Upload(upload)) => {
                base::InputMedia::InputMediaUploadedPhoto(Box::new(types::InputMediaUploadedPhoto {
                    spoiler: false,
                    live_photo: false,
                    file: self.save_file(*upload).await?,
                    stickers: None,
                    ttl_seconds: None,
                    video: None,
                }))
            }
        };
        match self.upload_media(peer, photo, business_connection_id).await? {
            base::InputMedia::InputMediaPhoto(p) => Ok(Some(p.id)),
            _ => Err(Error::Unexpected("messages.uploadMedia returned no photo for the cover")),
        }
    }
}

/// Telegram tells a round video, a voice message or a looping GIF from a plain file only by these attributes.
fn uploaded_document(
    kind: MediaKind,
    media: &InputMedia<'_>,
    upload: Upload<'_>,
    file: base::InputFile,
) -> types::InputMediaUploadedDocument {
    let (duration, w, h) = (media.duration, media.width, media.height);
    let fallback = match kind {
        MediaKind::Video | MediaKind::Animation | MediaKind::VideoNote | MediaKind::LivePhoto => "video/mp4",
        MediaKind::Audio => "audio/mpeg",
        MediaKind::Voice => "audio/ogg",
        MediaKind::Sticker => "image/webp",
        _ => "application/octet-stream",
    };
    let video = |round_message, supports_streaming| {
        base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
            round_message,
            supports_streaming,
            nosound: false,
            duration: f64::from(duration),
            w,
            h,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }))
    };
    let mut attributes = match kind {
        MediaKind::Video | MediaKind::LivePhoto => vec![video(false, media.supports_streaming)],
        MediaKind::Animation => vec![video(false, false), base::DocumentAttribute::DocumentAttributeAnimated],
        MediaKind::VideoNote => vec![video(true, false)],
        MediaKind::Audio | MediaKind::Voice => {
            vec![base::DocumentAttribute::DocumentAttributeAudio(Box::new(types::DocumentAttributeAudio {
                voice: kind == MediaKind::Voice,
                duration,
                title: media.title.clone(),
                performer: media.performer.clone(),
                waveform: None,
            }))]
        }
        MediaKind::Sticker => {
            vec![base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
                mask: false,
                alt: media.emoji.clone(),
                stickerset: base::InputStickerSet::InputStickerSetEmpty,
                mask_coords: None,
            }))]
        }
        _ => Vec::new(),
    };
    if !matches!(kind, MediaKind::Voice | MediaKind::VideoNote) {
        let file_name = types::DocumentAttributeFilename { file_name: upload.name().into() };
        attributes.push(base::DocumentAttribute::DocumentAttributeFilename(Box::new(file_name)));
    }
    types::InputMediaUploadedDocument {
        nosound_video: kind == MediaKind::Animation,
        force_file: media.force_file,
        spoiler: media.spoiler,
        file,
        thumb: None,
        mime_type: match upload.mime_type() {
            "application/octet-stream" => fallback,
            mime => mime,
        }
        .into(),
        attributes,
        stickers: None,
        video_cover: None,
        video_timestamp: None,
        ttl_seconds: None,
    }
}

fn stored_media(stored: base::MessageMedia) -> Option<base::InputMedia> {
    match stored {
        base::MessageMedia::MessageMediaPhoto(m) => {
            let video = m.video.and_then(input_document_of);
            Some(base::InputMedia::InputMediaPhoto(Box::new(types::InputMediaPhoto {
                spoiler: m.spoiler,
                live_photo: m.live_photo,
                id: input_photo_of(m.photo?)?,
                ttl_seconds: None,
                video,
            })))
        }
        base::MessageMedia::MessageMediaDocument(m) => {
            Some(base::InputMedia::InputMediaDocument(Box::new(types::InputMediaDocument {
                spoiler: m.spoiler,
                id: input_document_of(m.document?)?,
                video_cover: m.video_cover.and_then(input_photo_of),
                video_timestamp: m.video_timestamp,
                ttl_seconds: None,
                query: None,
            })))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{stored_media, uploaded_document};
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::messages_and_media::Source;
    use crate::types::messages_and_media::media::tests::document_media;
    use crate::types::{InputMedia, Upload};

    #[tokio::test]
    async fn uploads_keep_their_flags_and_the_business_connection() {
        let (client, mut sent) = Client::scripted(vec![Ok(document_media("video/mp4", Vec::new()).to_bytes())]);
        let cover = types::InputPhoto { id: 7, access_hash: 8, file_reference: vec![9] };
        let mut uploaded = document(InputMedia::video("a.mp4"));
        uploaded.spoiler = true;
        (uploaded.video_cover, uploaded.video_timestamp) =
            (Some(base::InputPhoto::InputPhoto(Box::new(cover.clone()))), Some(5));
        let media = base::InputMedia::InputMediaUploadedDocument(Box::new(uploaded));
        let peer = base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: 5 }));

        let stored = client.upload_media(&peer, media, Some("conn")).await.unwrap();
        let base::InputMedia::InputMediaDocument(d) = stored else { panic!("not a stored document") };
        assert!(d.spoiler);
        assert_eq!((d.video_cover, d.video_timestamp), (Some(base::InputPhoto::InputPhoto(Box::new(cover))), Some(5)));
        let body = sent.try_recv().unwrap();
        assert!(body.windows(5).any(|w| w == b"\x04conn"), "uploadMedia went out without the connection");
    }

    fn document(media: InputMedia<'_>) -> types::InputMediaUploadedDocument {
        let Source::Upload(upload) = media.file.0 else { panic!("not an upload") };
        let file = types::InputFile { id: 1, parts: 1, name: String::new(), md5_checksum: String::new() };
        uploaded_document(media.kind, &media, upload, base::InputFile::InputFile(Box::new(file)))
    }

    fn video(round_message: bool, supports_streaming: bool, duration: f64, w: i32, h: i32) -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
            round_message,
            supports_streaming,
            nosound: false,
            duration,
            w,
            h,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }))
    }

    fn name(file_name: &str) -> base::DocumentAttribute {
        let name = types::DocumentAttributeFilename { file_name: file_name.into() };
        base::DocumentAttribute::DocumentAttributeFilename(Box::new(name))
    }

    #[test]
    fn video_kinds_get_their_video_attributes() {
        let sent = document(InputMedia::video("clip.MP4").duration(7).width(640).height(360).supports_streaming());
        assert_eq!((sent.mime_type.as_str(), sent.nosound_video), ("video/mp4", false));
        assert_eq!(sent.attributes, [video(false, true, 7.0, 640, 360), name("clip.MP4")]);

        let note = document(InputMedia::video_note(Upload::bytes("note", b"x")).length(240).duration(3));
        assert_eq!((note.mime_type.as_str(), note.attributes), ("video/mp4", vec![video(true, false, 3.0, 240, 240)]));

        let gif = document(InputMedia::animation("cat.gif").has_spoiler());
        assert_eq!((gif.mime_type.as_str(), gif.nosound_video, gif.spoiler), ("image/gif", true, true));
        let animated = base::DocumentAttribute::DocumentAttributeAnimated;
        assert_eq!(gif.attributes, [video(false, false, 0.0, 0, 0), animated, name("cat.gif")]);
    }

    #[test]
    fn audio_kinds_get_their_audio_attributes() {
        let audio = |voice, title: Option<&str>, performer: Option<&str>| {
            base::DocumentAttribute::DocumentAttributeAudio(Box::new(types::DocumentAttributeAudio {
                voice,
                duration: 42,
                title: title.map(Into::into),
                performer: performer.map(Into::into),
                waveform: None,
            }))
        };
        let song = document(InputMedia::audio("song.mp3").duration(42).performer("Band").title("Song"));
        assert_eq!(song.mime_type, "audio/mpeg");
        assert_eq!(song.attributes, [audio(false, Some("Song"), Some("Band")), name("song.mp3")]);

        let voice = document(InputMedia::voice(Upload::bytes("voice", b"x")).duration(42));
        assert_eq!((voice.mime_type.as_str(), voice.attributes), ("audio/ogg", vec![audio(true, None, None)]));
    }

    #[test]
    fn stickers_and_documents_keep_their_names() {
        let sticker = document(InputMedia::sticker("hi.tgs").emoji("👋"));
        let attribute = types::DocumentAttributeSticker {
            mask: false,
            alt: "👋".into(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        };
        let attribute = base::DocumentAttribute::DocumentAttributeSticker(Box::new(attribute));
        assert_eq!(sticker.mime_type, "application/x-tgsticker");
        assert_eq!(sticker.attributes, [attribute, name("hi.tgs")]);
        assert_eq!(document(InputMedia::sticker(Upload::bytes("s", b"x"))).mime_type, "image/webp");

        let file = document(InputMedia::document("report.PDF"));
        assert_eq!((file.mime_type.as_str(), file.attributes), ("application/pdf", vec![name("report.PDF")]));
        let unknown = ["data.xyz", "README"].map(|n| document(InputMedia::document(n)).mime_type);
        assert_eq!(unknown, ["application/octet-stream"; 2]);
    }

    #[test]
    fn stored_media_becomes_a_reference() {
        let photo = types::Photo {
            has_stickers: false,
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 0,
            sizes: Vec::new(),
            video_sizes: None,
            dc_id: 2,
        };
        let stored = types::MessageMediaPhoto {
            spoiler: true,
            live_photo: false,
            photo: Some(base::Photo::Photo(Box::new(photo))),
            ttl_seconds: None,
            video: None,
        };
        let Some(base::InputMedia::InputMediaPhoto(p)) =
            stored_media(base::MessageMedia::MessageMediaPhoto(Box::new(stored)))
        else {
            panic!("not a stored photo")
        };
        let id = types::InputPhoto { id: 1, access_hash: 2, file_reference: vec![3] };
        let id = base::InputPhoto::InputPhoto(Box::new(id));
        assert_eq!((p.spoiler, p.live_photo, &p.id, &p.video), (true, false, &id, &None));

        let document = types::Document {
            id: 4,
            access_hash: 5,
            file_reference: vec![6],
            date: 0,
            mime_type: "video/mp4".into(),
            size: 1,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: Vec::new(),
        };
        let stored = types::MessageMediaDocument {
            nopremium: false,
            spoiler: false,
            video: true,
            round: false,
            voice: false,
            document: Some(base::Document::Document(Box::new(document))),
            alt_documents: None,
            video_cover: None,
            video_timestamp: Some(9),
            ttl_seconds: None,
        };
        let Some(base::InputMedia::InputMediaDocument(d)) =
            stored_media(base::MessageMedia::MessageMediaDocument(Box::new(stored)))
        else {
            panic!("not a stored document")
        };
        let id = types::InputDocument { id: 4, access_hash: 5, file_reference: vec![6] };
        assert_eq!(d.id, base::InputDocument::InputDocument(Box::new(id)));
        assert_eq!((d.spoiler, d.video_timestamp, &d.video_cover), (false, Some(9), &None));
        assert!(stored_media(base::MessageMedia::MessageMediaEmpty).is_none());
    }
}
