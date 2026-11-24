//! Files for a client in Bot API mode: sends and edits with media, albums, copies, downloads, and the replied
//! messages updates bring along. These are friendly methods' own branches, not TL translations: an upload goes in
//! the multipart request of the method that sends it, so TL's `upload.saveFilePart` flow never starts.

use std::cell::Cell;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::bridge::{Transport, options as send_flags, reply_parameters, unsupported};
use super::client::{FileBody, invalid};
use super::convert::{self, int};
use super::input_file::Attachment;
use super::raw::types as bot;
use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::{FileType, MediaKind, ParseMode};
use crate::errors::{Error, Result};
use crate::file_id::{FileId, ThumbnailSource};
use crate::methods::messages::SendOptions;
use crate::raw::{base, types};
use crate::session::internals::now;
use crate::types::messages_and_media::Source;
use crate::types::{FileLocation, FileSource, InputMedia, Message, PeerRef, ReplyMarkup, Text, Upload};
use crate::utils::{get_chat_id, get_peer, get_peer_id};

/// What `getFile` on `api.telegram.org` refuses: files over 20 MB.
const TOO_BIG: &str = "the Bot API downloads files of up to 20 MB; use MTProto or a local Bot API server";

/// The uploads of one request, each named by its `attach://` key.
#[derive(Default)]
struct Files(Vec<Attachment>);

impl Files {
    /// What names `file` in a request: its file id or URL, or `attach://<key>` for bytes that go along as a part of
    /// the request, under `name` when one is set.
    async fn add(&mut self, file: &FileSource<'_>, name: Option<&str>) -> Result<Value> {
        match &file.0 {
            Source::Url(url) => Ok(url.clone().into()),
            Source::Stored(_) => {
                let id = file.1.clone().ok_or(Error::InvalidArgument("this stored file has no Bot API file id"))?;
                Ok(id.into())
            }
            Source::Upload(upload) => self.upload(*upload, name).await,
            Source::Uploaded(_) => Err(unsupported("sending a file uploaded over MTProto")),
        }
    }

    async fn upload(&mut self, upload: Upload<'_>, name: Option<&str>) -> Result<Value> {
        let data: Arc<[u8]> = match upload {
            Upload::Path(path) => tokio::fs::read(path).await?.into(),
            Upload::Bytes { data, .. } => data.into(),
        };
        let key = format!("file{}", self.0.len());
        let value = Value::from(format!("attach://{key}"));
        self.0.push((key, name.unwrap_or(upload.name()).to_owned(), data));
        Ok(value)
    }

    /// Puts `media` into `out`: its file under `keys.0`, a live photo's video under `keys.1`, its caption, and the
    /// options its kind takes in the Bot API. A voice note's waveform has no Bot API field and is left out.
    async fn describe(
        &mut self,
        client: &Client,
        me: i64,
        media: InputMedia<'_>,
        out: &mut Map<String, Value>,
        keys: (&str, &str),
    ) -> Result<()> {
        let kind = media.kind;
        if media.mask_position.is_some() {
            return Err(unsupported("sending a sticker with a mask position"));
        }
        out.insert(keys.0.into(), self.add(&media.file, media.file_name.as_deref()).await?);
        if let Some(video) = &media.video {
            out.insert(keys.1.into(), self.add(video, None).await?);
        }
        if !matches!(kind, MediaKind::VideoNote | MediaKind::Sticker) {
            let (caption, entities) = client.write_text(media.caption, media.parse_mode).await?;
            if !caption.is_empty() {
                out.insert("caption".into(), caption.into());
            }
            if !entities.is_empty() {
                out.insert("caption_entities".into(), convert::entities_json(&entities, me));
            }
        }
        let visual = matches!(kind, MediaKind::Photo | MediaKind::LivePhoto | MediaKind::Video | MediaKind::Animation);
        flag(out, "show_caption_above_media", visual && media.show_caption_above_media);
        flag(out, "has_spoiler", visual && media.spoiler);
        flag(out, "supports_streaming", kind == MediaKind::Video && media.supports_streaming);
        flag(out, "disable_content_type_detection", kind == MediaKind::Document && media.force_file);
        let timed = matches!(kind, MediaKind::Video | MediaKind::Animation | MediaKind::Audio | MediaKind::Voice);
        if timed || kind == MediaKind::VideoNote {
            positive(out, "duration", media.duration);
        }
        if matches!(kind, MediaKind::Video | MediaKind::Animation) {
            positive(out, "width", media.width);
            positive(out, "height", media.height);
        }
        if kind == MediaKind::VideoNote {
            positive(out, "length", media.width);
        }
        if kind == MediaKind::Video {
            if let Some(start) = media.start_timestamp {
                out.insert("start_timestamp".into(), start.into());
            }
            if let Some(cover) = &media.cover {
                out.insert("cover".into(), self.add(cover, None).await?);
            }
        }
        if kind == MediaKind::Audio {
            if let Some(performer) = media.performer {
                out.insert("performer".into(), performer.into());
            }
            if let Some(title) = media.title {
                out.insert("title".into(), title.into());
            }
        }
        if kind == MediaKind::Sticker && !media.emoji.is_empty() {
            out.insert("emoji".into(), media.emoji.into());
        }
        let thumbed = !matches!(kind, MediaKind::Photo | MediaKind::LivePhoto | MediaKind::Voice | MediaKind::Sticker);
        if let Some(thumb) = media.thumb.filter(|_| thumbed) {
            out.insert("thumbnail".into(), self.upload(thumb, None).await?);
        }
        Ok(())
    }
}

/// Sets `key` when `on`: the Bot API reads a missing flag as false.
fn flag(out: &mut Map<String, Value>, key: &str, on: bool) {
    if on {
        out.insert(key.into(), true.into());
    }
}

/// Sets `key` to `value` when it is above 0, which stands for unknown.
fn positive(out: &mut Map<String, Value>, key: &str, value: i32) {
    if value > 0 {
        out.insert(key.into(), value.into());
    }
}

/// The Bot API method that sends one file of `kind`, and the field the file goes in.
fn send_method(kind: MediaKind) -> (&'static str, &'static str) {
    match kind {
        MediaKind::Photo => ("sendPhoto", "photo"),
        MediaKind::LivePhoto => ("sendLivePhoto", "photo"),
        MediaKind::Video => ("sendVideo", "video"),
        MediaKind::Animation => ("sendAnimation", "animation"),
        MediaKind::Audio => ("sendAudio", "audio"),
        MediaKind::Voice => ("sendVoice", "voice"),
        MediaKind::VideoNote => ("sendVideoNote", "video_note"),
        MediaKind::Sticker => ("sendSticker", "sticker"),
        _ => ("sendDocument", "document"),
    }
}

/// An album item or a message's new media as the Bot API's `InputMedia` JSON: its `type`, and the field its file
/// goes in.
fn input_media_type(kind: MediaKind) -> Result<(&'static str, &'static str)> {
    Ok(match kind {
        MediaKind::Photo => ("photo", "media"),
        // `media` is a live photo's video, `photo` its still.
        MediaKind::LivePhoto => ("live_photo", "photo"),
        MediaKind::Video => ("video", "media"),
        MediaKind::Animation => ("animation", "media"),
        MediaKind::Audio => ("audio", "media"),
        MediaKind::Document => ("document", "media"),
        _ => return Err(Error::InvalidArgument("this kind of media can't go in an album or a media edit")),
    })
}

/// `media` as an album item or a message's new media.
async fn input_media(files: &mut Files, client: &Client, me: i64, media: InputMedia<'_>) -> Result<Value> {
    let (kind, key) = input_media_type(media.kind)?;
    let mut out = Map::new();
    out.insert("type".into(), kind.into());
    files.describe(client, me, media, &mut out, (key, "media")).await?;
    Ok(Value::Object(out))
}

/// The chat a request names, without a lookup for an id.
async fn resolve_chat(client: &Client, chat: &PeerRef) -> Result<i64> {
    match chat {
        PeerRef::Id(id) => Ok(*id),
        other => Ok(client.resolve(other).await?.0),
    }
}

impl Transport {
    /// The fields every send shares: the chat, topic, flags, reply, keyboard, effect and business connection.
    async fn send_body(&self, client: &Client, chat_id: i64, options: &SendOptions) -> Result<Map<String, Value>> {
        if options.schedule_date.is_some() {
            return Err(unsupported("scheduled messages"));
        }
        if options.paid_message_star_count.is_some() || options.suggested_post_parameters.is_some() {
            return Err(unsupported("paid messages and suggested posts"));
        }
        if options.direct_messages_topic_id.is_some() {
            return Err(unsupported("sending into a direct messages topic"));
        }
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), chat_id.into());
        if let Some(id) = &options.business_connection_id {
            body.insert("business_connection_id".into(), id.clone().into());
        }
        if let Some(thread) = options.thread() {
            body.insert("message_thread_id".into(), thread.into());
        }
        send_flags(&mut body, options.disable_notification, options.protect_content, options.allow_paid_broadcast);
        if let Some(reply) = &options.reply_to {
            reply_parameters(&mut body, &reply.write(client, chat_id).await?, me)?;
        }
        if let Some(markup) = ReplyMarkup::write_option(options.reply_markup.as_ref(), client).await? {
            body.insert("reply_markup".into(), convert::markup_json(&markup, me)?);
        }
        if let Some(effect) = options.message_effect_id {
            body.insert("message_effect_id".into(), effect.to_string().into());
        }
        Ok(body)
    }

    /// The friendly message of one the Bot API returned after a send or an edit.
    fn message(&self, client: &Client, chat_id: i64, sent: Value, edit: bool) -> Result<Message> {
        let updates = base::Updates::Updates(Box::new(self.updates(vec![sent], edit)?));
        Message::from_updates(client, updates, Message::request(chat_id, String::new(), None))
    }

    /// `send_photo` and the other senders of one file: `sendPhoto` and the like, with an upload in the same request.
    pub(crate) async fn send_media(
        &self,
        client: &Client,
        chat: &PeerRef,
        media: InputMedia<'_>,
        options: SendOptions,
    ) -> Result<Message> {
        let chat_id = resolve_chat(client, chat).await?;
        let mut body = self.send_body(client, chat_id, &options).await?;
        let (method, key) = send_method(media.kind);
        let mut files = Files::default();
        files.describe(client, self.me(), media, &mut body, (key, "live_photo")).await?;
        let sent = self.api.call_with_files(method, Value::Object(body), files.0).await?;
        self.message(client, chat_id, sent, false)
    }

    /// `send_media_group`: one `sendMediaGroup` with every upload in it. The messages come back sorted by id.
    pub(crate) async fn send_media_group(
        &self,
        client: &Client,
        chat: &PeerRef,
        media: Vec<InputMedia<'_>>,
        options: SendOptions,
    ) -> Result<Vec<Message>> {
        let chat_id = resolve_chat(client, chat).await?;
        let mut body = self.send_body(client, chat_id, &options).await?;
        let me = self.me();
        let mut files = Files::default();
        let mut items = Vec::with_capacity(media.len());
        for item in media {
            items.push(input_media(&mut files, client, me, item).await?);
        }
        body.insert("media".into(), Value::Array(items));
        let Value::Array(sent) = self.api.call_with_files("sendMediaGroup", Value::Object(body), files.0).await? else {
            return Err(Error::Unexpected("sendMediaGroup returned no messages"));
        };
        let updates = base::Updates::Updates(Box::new(self.updates(sent, false)?));
        let mut messages: Vec<_> = Message::all_from_updates(client, updates).collect();
        if messages.is_empty() {
            return Err(Error::Unexpected("sendMediaGroup returned no messages"));
        }
        messages.sort_unstable_by_key(|m| m.id);
        Ok(messages)
    }

    /// `edit_message_media`: `editMessageMedia`, with an upload in the same request.
    pub(crate) async fn edit_message_media(
        &self,
        client: &Client,
        chat: &PeerRef,
        message_id: i32,
        media: InputMedia<'_>,
        reply_markup: Option<&ReplyMarkup>,
        business_connection_id: Option<&str>,
    ) -> Result<Message> {
        let chat_id = resolve_chat(client, chat).await?;
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), chat_id.into());
        body.insert("message_id".into(), message_id.into());
        if let Some(id) = business_connection_id {
            body.insert("business_connection_id".into(), id.into());
        }
        if let Some(markup) = ReplyMarkup::write_option(reply_markup, client).await? {
            body.insert("reply_markup".into(), convert::markup_json(&markup, me)?);
        }
        let mut files = Files::default();
        body.insert("media".into(), input_media(&mut files, client, me, media).await?);
        let edited = self.api.call_with_files("editMessageMedia", Value::Object(body), files.0).await?;
        self.message(client, chat_id, edited, true)
    }

    /// `copy_message`: `copyMessage` straight away, as the Bot API can't read the message first. It answers with
    /// the copy's id alone, so the message returned holds that id, the chat and a new caption, if one was set.
    pub(crate) async fn copy_message(
        &self,
        client: &Client,
        chat: &PeerRef,
        (from_chat, message_id): (&PeerRef, i32),
        caption: Option<(Text, Option<ParseMode>)>,
        video_start_timestamp: Option<i32>,
        options: SendOptions,
    ) -> Result<Message> {
        let chat_id = resolve_chat(client, chat).await?;
        let mut body = self.send_body(client, chat_id, &options).await?;
        body.insert("from_chat_id".into(), resolve_chat(client, from_chat).await?.into());
        body.insert("message_id".into(), message_id.into());
        let mut text = String::new();
        if let Some((caption, parse_mode)) = caption {
            let (caption, entities) = client.write_text(caption, parse_mode).await?;
            if !entities.is_empty() {
                body.insert("caption_entities".into(), convert::entities_json(&entities, self.me()));
            }
            body.insert("caption".into(), caption.clone().into());
            text = caption;
        }
        if let Some(seconds) = video_start_timestamp {
            body.insert("video_start_timestamp".into(), seconds.into());
        }
        let copied = self.api.call("copyMessage", &Value::Object(body)).await?;
        let copied = bot::MessageId::deserialize(&copied).map_err(invalid)?;
        let mut raw = types::Message::new(int(copied.message_id), get_peer(chat_id), int(now()), text);
        raw.out = true;
        self.saw(&raw);
        let updates = types::Updates {
            updates: vec![convert::message_update(raw, false)],
            users: Vec::new(),
            chats: Vec::new(),
            date: int(now()),
            seq: 0,
        };
        let sent = Message::request(chat_id, String::new(), None);
        Message::from_updates(client, base::Updates::Updates(Box::new(updates)), sent)
    }

    /// `getFile` for the file at `location`, then the start of its download from the file server, with the size
    /// `getFile` gave. A file over the limit of `api.telegram.org` fails here, before any byte is read.
    pub(crate) async fn open_file(&self, location: &FileLocation) -> Result<(FileBody, Option<i64>)> {
        let file_id = file_id_of(location)?;
        let file = match self.api.call("getFile", &json!({ "file_id": file_id })).await {
            Ok(file) => bot::File::deserialize(&file).map_err(invalid)?,
            Err(Error::Rpc(e)) if e.name == "FILE_IS_TOO_BIG" => return Err(Error::InvalidArgument(TOO_BIG)),
            Err(e) => return Err(e),
        };
        let path = file.file_path.ok_or(Error::Unexpected("getFile returned no file path"))?;
        Ok((self.api.fetch(&path).await?, file.file_size.or(location.file_size)))
    }
}

/// The Bot API file id of a download location, as the friendly types write their `file_id`: a document of any kind
/// goes as a plain document.
fn file_id_of(location: &FileLocation) -> Result<String> {
    let dc_id = location.dc_id.ok_or(Error::InvalidArgument("the file's data center is unknown"))?;
    let id = match &location.location {
        base::InputFileLocation::InputPhotoFileLocation(l) => FileId {
            thumbnail_source: sized(FileType::Photo, &l.thumb_size),
            ..FileId::new(FileType::Photo, dc_id, l.id, l.access_hash, &l.file_reference)
        },
        base::InputFileLocation::InputDocumentFileLocation(l) if l.thumb_size.is_empty() => {
            FileId::new(FileType::Document, dc_id, l.id, l.access_hash, &l.file_reference)
        }
        base::InputFileLocation::InputDocumentFileLocation(l) => FileId {
            thumbnail_source: sized(FileType::Thumbnail, &l.thumb_size),
            ..FileId::new(FileType::Thumbnail, dc_id, l.id, l.access_hash, &l.file_reference)
        },
        _ => return Err(unsupported("downloading chat photos and sticker set thumbnails")),
    };
    Ok(id.encode())
}

/// The size `thumb_size` of a file of `file_type`, as a file id names it; `None` for the file itself.
fn sized(file_type: FileType, thumb_size: &str) -> Option<ThumbnailSource> {
    thumb_size.chars().next().map(|size| ThumbnailSource::Thumbnail { file_type, size })
}

thread_local! {
    // Set while a carried message is built: its own reply isn't looked up, as the Bot API carries one level only, and
    // a chain of replies kept one under another would otherwise be built all the way down.
    static CARRYING: Cell<bool> = const { Cell::new(false) };
}

/// The replied message the Bot API sent along with `raw`, which `Message::reply_to_message` returns; `None` for a
/// client in another mode, a message that replies to nothing, or a reply that came without it.
pub(crate) fn carried_reply(client: &Client, raw: &types::Message) -> Option<Box<Message>> {
    raw.reply_to.as_ref()?;
    if CARRYING.get() {
        return None;
    }
    let carried = client.0.bot_api.as_ref()?.carried(get_peer_id(&raw.peer_id), raw.id)?;
    // Built as it is, without `Entities::new`: carried users and chats are `min`, with no hash to remember, and a
    // message can be built while the peer cache is locked.
    let mut entities = Entities::default();
    for user in &carried.users {
        if let base::User::User(u) = user {
            entities.users.insert(u.id, user.clone());
        }
    }
    for chat in &carried.chats {
        if let Some(id) = get_chat_id(chat) {
            entities.chats.insert(id, chat.clone());
        }
    }
    // Building is synchronous, so the flag covers exactly this message.
    CARRYING.set(true);
    let message = Message::parse(client, Box::new(carried.message.clone()), &entities);
    CARRYING.set(false);
    Some(Box::new(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_become_the_file_ids_the_friendly_types_show() {
        let photo = FileId {
            thumbnail_source: Some(ThumbnailSource::Thumbnail { file_type: FileType::Photo, size: 'y' }),
            ..FileId::new(FileType::Photo, 2, 555, 666, &[1, 2])
        };
        let location = FileLocation { file_size: Some(9), ..FileLocation::from(&photo) };
        assert_eq!(file_id_of(&location).unwrap(), photo.encode());

        let document = FileId::new(FileType::Document, 4, 777, 888, &[3]);
        assert_eq!(file_id_of(&FileLocation::from(&document)).unwrap(), document.encode());

        let mut unknown = FileLocation::from(&document);
        unknown.dc_id = None;
        assert!(matches!(file_id_of(&unknown), Err(Error::InvalidArgument(_))));
    }

    #[test]
    fn only_album_kinds_go_in_albums_and_edits() {
        assert_eq!(input_media_type(MediaKind::LivePhoto).unwrap(), ("live_photo", "photo"));
        assert_eq!(input_media_type(MediaKind::Document).unwrap(), ("document", "media"));
        assert!(input_media_type(MediaKind::Voice).is_err());
        assert_eq!(send_method(MediaKind::VideoNote), ("sendVideoNote", "video_note"));
    }
}
