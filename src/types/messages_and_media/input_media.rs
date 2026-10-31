use crate::enums::{MediaKind, ParseMode};
use crate::types::messages_and_media::Source;
use crate::types::{FileSource, Text, Upload};

#[derive(Debug, Clone)]
pub struct InputMedia<'a> {
    pub(crate) kind: MediaKind,
    pub(crate) file: FileSource<'a>,
    pub(crate) thumb: Option<Upload<'a>>,
    pub(crate) cover: Option<FileSource<'a>>,
    pub(crate) video: Option<FileSource<'a>>,
    pub(crate) caption: Text,
    pub(crate) parse_mode: Option<ParseMode>,
    pub(crate) spoiler: bool,
    pub(crate) show_caption_above_media: bool,
    pub(crate) duration: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) supports_streaming: bool,
    pub(crate) start_timestamp: Option<i32>,
    pub(crate) performer: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) emoji: String,
    pub(crate) force_file: bool,
}

impl<'a> InputMedia<'a> {
    fn new(kind: MediaKind, file: impl Into<FileSource<'a>>) -> Self {
        InputMedia {
            kind,
            file: file.into(),
            thumb: None,
            cover: None,
            video: None,
            caption: Text::new(),
            parse_mode: None,
            spoiler: false,
            show_caption_above_media: false,
            duration: 0,
            width: 0,
            height: 0,
            supports_streaming: false,
            start_timestamp: None,
            performer: None,
            title: None,
            emoji: String::new(),
            force_file: false,
        }
    }

    pub fn photo(photo: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Photo, photo)
    }

    pub fn live_photo(photo: impl Into<FileSource<'a>>, video: impl Into<FileSource<'a>>) -> Self {
        InputMedia { video: Some(video.into()), ..Self::new(MediaKind::LivePhoto, photo) }
    }

    pub fn video(video: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Video, video)
    }

    pub fn animation(animation: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Animation, animation)
    }

    pub fn audio(audio: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Audio, audio)
    }

    pub fn voice(voice: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Voice, voice)
    }

    pub fn document(document: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Document, document)
    }

    pub fn video_note(video_note: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::VideoNote, video_note)
    }

    pub fn sticker(sticker: impl Into<FileSource<'a>>) -> Self {
        Self::new(MediaKind::Sticker, sticker)
    }

    pub(crate) fn uploads(&self) -> bool {
        let upload = |f: &FileSource<'_>| matches!(f.0, Source::Upload(_) | Source::Uploaded(_));
        upload(&self.file) || self.cover.as_ref().is_some_and(upload) || self.video.as_ref().is_some_and(upload)
    }
}

macro_rules! media_setters {
    ($builder:ident . $media:ident: $($setter:ident)*) => {
        $(media_setters!(@$setter $builder . $media);)*
    };
    ($builder:ident: $($setter:ident)*) => {
        $(media_setters!(@$setter $builder);)*
    };
    (@caption $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn caption(mut self, caption: impl Into<$crate::types::Text>) -> Self {
                self$(.$m)?.caption = caption.into();
                self
            }
        }

        $crate::methods::caption_setters!($builder $(. $m)?);
    };
    (@thumb $builder:ident $(. $m:ident)?) => {
        impl<'a> $builder<'a> {
            /// A JPEG of at most 320×320 and 200 KB; Telegram ignores anything bigger.
            pub fn thumb(mut self, thumb: impl Into<$crate::types::Upload<'a>>) -> Self {
                self$(.$m)?.thumb = Some(thumb.into());
                self
            }
        }
    };
    (@cover $builder:ident $(. $m:ident)?) => {
        impl<'a> $builder<'a> {
            pub fn cover(mut self, cover: impl Into<$crate::types::FileSource<'a>>) -> Self {
                self$(.$m)?.cover = Some(cover.into());
                self
            }
        }
    };
    (@has_spoiler $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn has_spoiler(mut self) -> Self {
                self$(.$m)?.spoiler = true;
                self
            }
        }
    };
    (@show_caption_above_media $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn show_caption_above_media(mut self) -> Self {
                self$(.$m)?.show_caption_above_media = true;
                self
            }
        }
    };
    (@supports_streaming $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn supports_streaming(mut self) -> Self {
                self$(.$m)?.supports_streaming = true;
                self
            }
        }
    };
    (@duration $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn duration(mut self, seconds: i32) -> Self {
                self$(.$m)?.duration = seconds;
                self
            }
        }
    };
    (@start_timestamp $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn start_timestamp(mut self, seconds: i32) -> Self {
                self$(.$m)?.start_timestamp = Some(seconds);
                self
            }
        }
    };
    (@width $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn width(mut self, width: i32) -> Self {
                self$(.$m)?.width = width;
                self
            }
        }
    };
    (@height $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn height(mut self, height: i32) -> Self {
                self$(.$m)?.height = height;
                self
            }
        }
    };
    (@length $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn length(mut self, length: i32) -> Self {
                (self$(.$m)?.width, self$(.$m)?.height) = (length, length);
                self
            }
        }
    };
    (@performer $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn performer(mut self, performer: impl Into<String>) -> Self {
                self$(.$m)?.performer = Some(performer.into());
                self
            }
        }
    };
    (@title $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn title(mut self, title: impl Into<String>) -> Self {
                self$(.$m)?.title = Some(title.into());
                self
            }
        }
    };
    (@emoji $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            pub fn emoji(mut self, emoji: impl Into<String>) -> Self {
                self$(.$m)?.emoji = emoji.into();
                self
            }
        }
    };
    (@disable_content_type_detection $builder:ident $(. $m:ident)?) => {
        impl $builder<'_> {
            /// Keeps a video, GIF or audio file a plain document instead of letting Telegram convert it.
            pub fn disable_content_type_detection(mut self) -> Self {
                self$(.$m)?.force_file = true;
                self
            }
        }
    };
    (@progress $builder:ident $(. $m:ident)?) => {
        impl<'a> $builder<'a> {
            pub fn progress(mut self, progress: impl FnMut(u64, u64) + Send + 'a) -> Self {
                self.progress = Some(Box::new(progress));
                self
            }
        }
    };
}
pub(crate) use media_setters;

media_setters!(InputMedia: caption thumb cover has_spoiler show_caption_above_media supports_streaming duration
    start_timestamp width height length performer title emoji disable_content_type_detection);

#[cfg(test)]
mod tests {
    use super::InputMedia;
    use crate::raw::{base, types};
    use crate::types::{FileSource, UploadedFile};

    #[test]
    fn an_uploaded_file_counts_as_an_upload() {
        let file = types::InputFile { id: 7, parts: 1, name: "a.jpg".into(), md5_checksum: String::new() };
        let file = UploadedFile(base::InputFile::InputFile(Box::new(file)));
        let url = || FileSource::url("https://example.com/a.mp4").unwrap();
        assert!(!InputMedia::video(url()).uploads());
        assert!(InputMedia::photo(&file).uploads());
        assert!(InputMedia::video(url()).cover(&file).uploads());
    }
}
