use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::messages_and_media::{input_document, input_photo};
use crate::types::{
    FileLocation, InlineQueryResultArticle, InlineQueryResultAudio, InlineQueryResultCachedAudio,
    InlineQueryResultCachedDocument, InlineQueryResultCachedGif, InlineQueryResultCachedMpeg4Gif,
    InlineQueryResultCachedPhoto, InlineQueryResultCachedSticker, InlineQueryResultCachedVideo,
    InlineQueryResultCachedVoice, InlineQueryResultContact, InlineQueryResultDocument, InlineQueryResultGame,
    InlineQueryResultGif, InlineQueryResultLocation, InlineQueryResultMpeg4Gif, InlineQueryResultPhoto,
    InlineQueryResultVenue, InlineQueryResultVideo, InlineQueryResultVoice, InputMessageContent, Photo, ReplyMarkup,
};

use super::input_message_content::{web_document, web_photo};

macro_rules! results {
    ($($variant:ident($result:ident) => $kind:literal,)*) => {
        #[derive(Debug, Clone, PartialEq)]
        #[non_exhaustive]
        pub enum InlineQueryResult {
            $($variant($result),)*
        }

        $(impl From<$result> for InlineQueryResult {
            fn from(result: $result) -> Self {
                InlineQueryResult::$variant(result)
            }
        })*

        impl InlineQueryResult {
            fn parts(self) -> (&'static str, Parts) {
                match self {
                    $(InlineQueryResult::$variant(r) => ($kind, r.parts()),)*
                }
            }
        }
    };
}

results! {
    Article(InlineQueryResultArticle) => "article",
    Photo(InlineQueryResultPhoto) => "photo",
    Gif(InlineQueryResultGif) => "gif",
    Mpeg4Gif(InlineQueryResultMpeg4Gif) => "gif",
    Video(InlineQueryResultVideo) => "video",
    Audio(InlineQueryResultAudio) => "audio",
    Voice(InlineQueryResultVoice) => "voice",
    Document(InlineQueryResultDocument) => "file",
    Location(InlineQueryResultLocation) => "geo",
    Venue(InlineQueryResultVenue) => "venue",
    Contact(InlineQueryResultContact) => "contact",
    Game(InlineQueryResultGame) => "game",
    CachedPhoto(InlineQueryResultCachedPhoto) => "photo",
    CachedGif(InlineQueryResultCachedGif) => "gif",
    CachedMpeg4Gif(InlineQueryResultCachedMpeg4Gif) => "gif",
    CachedSticker(InlineQueryResultCachedSticker) => "sticker",
    CachedDocument(InlineQueryResultCachedDocument) => "file",
    CachedVideo(InlineQueryResultCachedVideo) => "video",
    CachedVoice(InlineQueryResultCachedVoice) => "voice",
    CachedAudio(InlineQueryResultCachedAudio) => "audio",
}

/// Setters shared by several result kinds; each kind lists only the ones the Bot API gives it.
macro_rules! setters {
    ($result:ident: $($setter:ident),* $(,)?) => {
        impl $result {
            $(setters!(@$setter);)*
        }
    };
    (@id) => {
        /// Unique among the results of one answer; random unless set.
        pub fn id(mut self, id: impl Into<String>) -> Self {
            self.parts.id = id.into();
            self
        }
    };
    (@reply_markup) => {
        pub fn reply_markup(mut self, markup: impl Into<$crate::types::ReplyMarkup>) -> Self {
            self.parts.reply_markup = Some(markup.into());
            self
        }
    };
    (@input_message_content) => {
        pub fn input_message_content(mut self, content: impl Into<$crate::types::InputMessageContent>) -> Self {
            self.parts.content = Some(content.into());
            self
        }
    };
    (@caption) => {
        pub fn caption(mut self, caption: impl Into<String>) -> Self {
            self.parts.caption = caption.into();
            self
        }

        pub fn parse_mode(mut self, mode: $crate::enums::ParseMode) -> Self {
            self.parts.parse_mode = Some(mode);
            self
        }

        pub fn html(self) -> Self {
            self.parse_mode($crate::enums::ParseMode::Html)
        }

        pub fn markdown(self) -> Self {
            self.parse_mode($crate::enums::ParseMode::Markdown)
        }
    };
    (@caption_above) => {
        pub fn show_caption_above_media(mut self) -> Self {
            self.parts.caption_above = true;
            self
        }
    };
    (@title) => {
        pub fn title(mut self, title: impl Into<String>) -> Self {
            self.parts.title = Some(title.into());
            self
        }
    };
    (@description) => {
        pub fn description(mut self, description: impl Into<String>) -> Self {
            self.parts.description = Some(description.into());
            self
        }
    };
    (@url) => {
        pub fn url(mut self, url: impl Into<String>) -> Self {
            self.parts.url = Some(url.into());
            self
        }
    };
    (@thumbnail) => {
        pub fn thumbnail(mut self, thumbnail_url: impl Into<String>) -> Self {
            self.parts.thumbnail_url = Some(thumbnail_url.into());
            self
        }
    };
    (@dimensions) => {
        pub fn dimensions(mut self, width: i32, height: i32) -> Self {
            (self.file.width, self.file.height) = (width, height);
            self
        }
    };
    (@duration) => {
        pub fn duration(mut self, seconds: i32) -> Self {
            self.file.duration = seconds;
            self
        }
    };
    (@mime_type) => {
        pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
            self.file.mime_type = mime_type.into();
            self
        }
    };
    (@performer) => {
        pub fn performer(mut self, performer: impl Into<String>) -> Self {
            self.file.performer = Some(performer.into());
            self
        }
    };
}

pub(super) use setters;

#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Parts {
    pub(super) id: String,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) url: Option<String>,
    pub(super) thumbnail_url: Option<String>,
    pub(super) file: Option<WebFile>,
    pub(super) stored: Option<Stored>,
    pub(super) caption: String,
    pub(super) parse_mode: Option<ParseMode>,
    pub(super) caption_above: bool,
    pub(super) reply_markup: Option<ReplyMarkup>,
    pub(super) content: Option<InputMessageContent>,
}

impl Parts {
    pub(super) fn new() -> Parts {
        Parts { id: format!("{:016x}", u64::from_le_bytes(random())), ..Parts::default() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct WebFile {
    pub(super) url: String,
    pub(super) mime_type: String,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) duration: i32,
    pub(super) performer: Option<String>,
}

impl WebFile {
    pub(super) fn new(url: String, mime_type: &str) -> WebFile {
        WebFile { url, mime_type: mime_type.into(), width: 0, height: 0, duration: 0, performer: None }
    }
}

/// Media already on Telegram's servers, or a game registered with @BotFather.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Stored {
    Photo(base::InputPhoto),
    Document(base::InputDocument),
    Game(String),
}

impl Stored {
    pub(super) fn photo(photo: &Photo) -> Stored {
        Stored::Photo(input_photo(&photo.location))
    }

    pub(super) fn document(location: &FileLocation) -> Stored {
        Stored::Document(input_document(location))
    }
}

impl InlineQueryResult {
    pub(crate) async fn write(self, client: &Client) -> Result<base::InputBotInlineResult> {
        let (kind, p) = self.parts();
        let reply_markup = p.reply_markup.as_ref().map(ReplyMarkup::write);
        let send_message = match p.content {
            Some(content) => content.write(client, reply_markup).await?,
            None if matches!(p.stored, Some(Stored::Game(_))) => {
                base::InputBotInlineMessage::InputBotInlineMessageGame(Box::new(types::InputBotInlineMessageGame {
                    reply_markup,
                }))
            }
            None => {
                let (message, entities) = client.parse_text(&p.caption, p.parse_mode).await?;
                base::InputBotInlineMessage::InputBotInlineMessageMediaAuto(Box::new(
                    types::InputBotInlineMessageMediaAuto {
                        invert_media: p.caption_above,
                        message,
                        entities: (!entities.is_empty()).then_some(entities),
                        reply_markup,
                    },
                ))
            }
        };
        Ok(match p.stored {
            None => {
                let content = p.file.map(|f| {
                    let attributes = attributes(kind, &f, p.title.as_deref());
                    web_document(f.url, f.mime_type, attributes)
                });
                base::InputBotInlineResult::InputBotInlineResult(Box::new(types::InputBotInlineResult {
                    id: p.id,
                    r#type: kind.into(),
                    title: p.title,
                    description: p.description,
                    url: p.url,
                    thumb: p.thumbnail_url.map(web_photo),
                    content,
                    send_message,
                }))
            }
            Some(Stored::Photo(photo)) => {
                base::InputBotInlineResult::InputBotInlineResultPhoto(Box::new(types::InputBotInlineResultPhoto {
                    id: p.id,
                    r#type: kind.into(),
                    photo,
                    send_message,
                }))
            }
            Some(Stored::Document(document)) => base::InputBotInlineResult::InputBotInlineResultDocument(Box::new(
                types::InputBotInlineResultDocument {
                    id: p.id,
                    r#type: kind.into(),
                    title: p.title,
                    description: p.description,
                    document,
                    send_message,
                },
            )),
            Some(Stored::Game(short_name)) => {
                base::InputBotInlineResult::InputBotInlineResultGame(Box::new(types::InputBotInlineResultGame {
                    id: p.id,
                    short_name,
                    send_message,
                }))
            }
        })
    }
}

fn attributes(kind: &str, file: &WebFile, title: Option<&str>) -> Vec<base::DocumentAttribute> {
    let attribute = match kind {
        "audio" | "voice" => base::DocumentAttribute::DocumentAttributeAudio(Box::new(types::DocumentAttributeAudio {
            voice: kind == "voice",
            duration: file.duration,
            title: title.filter(|_| kind == "audio").map(String::from),
            performer: file.performer.clone(),
            waveform: None,
        })),
        _ if file.mime_type.starts_with("video/") => {
            base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
                round_message: false,
                supports_streaming: false,
                nosound: false,
                duration: file.duration.into(),
                w: file.width,
                h: file.height,
                preload_prefix_size: None,
                video_start_ts: None,
                video_codec: None,
            }))
        }
        _ if file.width > 0 || file.height > 0 => {
            base::DocumentAttribute::DocumentAttributeImageSize(Box::new(types::DocumentAttributeImageSize {
                w: file.width,
                h: file.height,
            }))
        }
        _ => return Vec::new(),
    };
    vec![attribute]
}

#[cfg(test)]
mod tests {
    use super::{InlineQueryResult, attributes};
    use crate::raw::base;
    use crate::types::{
        InlineQueryResultAudio, InlineQueryResultDocument, InlineQueryResultLocation, InlineQueryResultMpeg4Gif,
        InputMessageContent,
    };

    #[test]
    fn kinds_pick_their_tl_type_and_media_attributes() {
        let audio = InlineQueryResultAudio::new("u", "Song").performer("Me");
        let (kind, parts) = InlineQueryResult::from(audio).parts();
        let file = parts.file.expect("audio results carry their file");
        let attrs = attributes(kind, &file, parts.title.as_deref());
        let [base::DocumentAttribute::DocumentAttributeAudio(a)] = attrs.as_slice() else {
            panic!("expected one audio attribute, got {attrs:?}")
        };
        assert_eq!(
            (kind, a.voice, a.title.as_deref(), a.performer.as_deref()),
            ("audio", false, Some("Song"), Some("Me"))
        );

        let gif = InlineQueryResultMpeg4Gif::new("u", "t").dimensions(2, 3);
        let (kind, parts) = InlineQueryResult::from(gif).parts();
        let file = parts.file.expect("mpeg4 results carry their file");
        let attrs = attributes(kind, &file, None);
        let [base::DocumentAttribute::DocumentAttributeVideo(v)] = attrs.as_slice() else {
            panic!("expected one video attribute, got {attrs:?}")
        };
        assert_eq!((kind, file.mime_type.as_str(), v.w, v.h), ("gif", "video/mp4", 2, 3));

        let (kind, parts) = InlineQueryResult::from(InlineQueryResultDocument::new("u", "t")).parts();
        let file = parts.file.expect("document results carry their file");
        assert_eq!((kind, attributes(kind, &file, None)), ("file", Vec::new()));
    }

    #[test]
    fn location_sends_itself_unless_overridden() {
        let location = InlineQueryResultLocation::new(1.0, 2.0, "Here").heading(90);
        let (kind, parts) = InlineQueryResult::from(location).parts();
        let Some(InputMessageContent::Location { latitude, heading, .. }) = parts.content else {
            panic!("expected location content, got {:?}", parts.content)
        };
        assert_eq!((kind, latitude, heading), ("geo", 1.0, Some(90)));

        let result = InlineQueryResultLocation::new(1.0, 2.0, "Here").input_message_content("text");
        let (_, parts) = InlineQueryResult::from(result).parts();
        assert!(matches!(parts.content, Some(InputMessageContent::Text { .. })));
    }
}
