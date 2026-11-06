use super::block::{Files, RichBlock, for_each_text, unzip};
use super::rich_text::push_mentions;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::messages_and_media::{input_document, input_photo};
use crate::types::{Document, Photo};

/// The content of a rich message to send, written as HTML, Markdown or a list of [`RichBlock`]s.
///
/// Every form carries a media list, the `rtl` flag and the `no_autolink` flag, which
/// [`media`](InputRichMessage::media), [`rtl`](InputRichMessage::rtl) and
/// [`no_autolink`](InputRichMessage::no_autolink) set.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InputRichMessage {
    /// HTML source, which refers to files added with [`media`](InputRichMessage::media) by their id.
    Html { html: String, media: Vec<InputRichMessageMedia>, rtl: bool, no_autolink: bool },
    /// Markdown source, which refers to files added with [`media`](InputRichMessage::media) by their id.
    Markdown { markdown: String, media: Vec<InputRichMessageMedia>, rtl: bool, no_autolink: bool },
    /// Blocks built in code. Users mentioned in their text are resolved when the message is sent.
    Blocks { blocks: Vec<RichBlock>, media: Vec<InputRichMessageMedia>, rtl: bool, no_autolink: bool },
}

/// A photo or document that HTML or Markdown content refers to by id, or that blocks include.
#[derive(Debug, Clone, PartialEq)]
pub struct InputRichMessageMedia(base::InputRichFile);

impl InputRichMessage {
    /// Creates content from HTML source.
    pub fn html(html: impl Into<String>) -> Self {
        InputRichMessage::Html { html: html.into(), media: Vec::new(), rtl: false, no_autolink: false }
    }

    /// Creates content from Markdown source.
    pub fn markdown(markdown: impl Into<String>) -> Self {
        InputRichMessage::Markdown { markdown: markdown.into(), media: Vec::new(), rtl: false, no_autolink: false }
    }

    /// Creates content from blocks. Files that media blocks show are attached automatically.
    pub fn blocks(blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        InputRichMessage::Blocks {
            blocks: blocks.into_iter().collect(),
            media: Vec::new(),
            rtl: false,
            no_autolink: false,
        }
    }

    /// Sets whether the message is laid out right to left; `true` lays it out right to left.
    pub fn rtl(mut self, rtl: bool) -> Self {
        match &mut self {
            InputRichMessage::Html { rtl: field, .. }
            | InputRichMessage::Markdown { rtl: field, .. }
            | InputRichMessage::Blocks { rtl: field, .. } => *field = rtl,
        }
        self
    }

    /// Sets whether automatic linking is off in the message; `true` turns it off.
    pub fn no_autolink(mut self, no_autolink: bool) -> Self {
        match &mut self {
            InputRichMessage::Html { no_autolink: field, .. }
            | InputRichMessage::Markdown { no_autolink: field, .. }
            | InputRichMessage::Blocks { no_autolink: field, .. } => *field = no_autolink,
        }
        self
    }

    /// Attaches one file to the message.
    ///
    /// HTML and Markdown content refer to the file by the string id given to the media. Blocks reference files by
    /// numeric id, so the string id is dropped for them.
    pub fn media(mut self, media: InputRichMessageMedia) -> Self {
        match &mut self {
            InputRichMessage::Html { media: list, .. }
            | InputRichMessage::Markdown { media: list, .. }
            | InputRichMessage::Blocks { media: list, .. } => list.push(media),
        }
        self
    }

    pub(crate) async fn write(self, client: &Client) -> Result<base::InputRichMessage> {
        let mut users = Vec::new();
        if let InputRichMessage::Blocks { blocks, .. } = &self {
            let mut ids = Vec::new();
            for block in blocks {
                for_each_text(&block.block, &mut |text| push_mentions(text, &mut ids));
            }
            ids.sort_unstable();
            ids.dedup();
            for id in ids {
                users.push(client.resolve_user(&id.into()).await?);
            }
        }
        Ok(self.build(users))
    }

    fn build(self, users: Vec<base::InputUser>) -> base::InputRichMessage {
        let files =
            |media: Vec<InputRichMessageMedia>| (!media.is_empty()).then(|| media.into_iter().map(|m| m.0).collect());
        match self {
            InputRichMessage::Html { html, media, rtl, no_autolink } => {
                let html = types::InputRichMessageHTML { rtl, noautolink: no_autolink, html, files: files(media) };
                base::InputRichMessage::InputRichMessageHTML(Box::new(html))
            }
            InputRichMessage::Markdown { markdown, media, rtl, no_autolink } => {
                let files = files(media);
                let markdown = types::InputRichMessageMarkdown { rtl, noautolink: no_autolink, markdown, files };
                base::InputRichMessage::InputRichMessageMarkdown(Box::new(markdown))
            }
            InputRichMessage::Blocks { blocks, media, rtl, no_autolink } => {
                let mut files = Files::default();
                let blocks = unzip(blocks, &mut files);
                for m in media {
                    match m.0 {
                        base::InputRichFile::InputRichFilePhoto(f) => files.add_photo(f.photo),
                        base::InputRichFile::InputRichFileDocument(f) => files.add_document(f.document),
                    };
                }
                let Files { photos, documents, .. } = files;
                base::InputRichMessage::InputRichMessage(Box::new(types::InputRichMessage {
                    rtl,
                    noautolink: no_autolink,
                    blocks,
                    photos: (!photos.is_empty()).then_some(photos),
                    documents: (!documents.is_empty()).then_some(documents),
                    users: (!users.is_empty()).then_some(users),
                }))
            }
        }
    }
}

impl InputRichMessageMedia {
    /// Wraps an existing [`Photo`] that the content refers to as `id`.
    pub fn photo(id: impl Into<String>, photo: &Photo) -> Self {
        let photo = input_photo(&photo.location);
        InputRichMessageMedia(base::InputRichFile::InputRichFilePhoto(Box::new(types::InputRichFilePhoto {
            id: id.into(),
            photo,
        })))
    }

    /// Wraps an existing [`Document`] that the content refers to as `id`.
    pub fn document(id: impl Into<String>, document: &Document) -> Self {
        let document = input_document(&document.location);
        InputRichMessageMedia(base::InputRichFile::InputRichFileDocument(Box::new(types::InputRichFileDocument {
            id: id.into(),
            document,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::{InputRichMessage, InputRichMessageMedia};
    use crate::Client;
    use crate::raw::{base, types};
    use crate::types::{FileLocation, Photo, RichBlock, RichListItem, Text};

    fn photo(id: i64) -> Photo {
        let location =
            types::InputPhotoFileLocation { id, access_hash: 1, file_reference: vec![], thumb_size: "x".into() };
        let location = FileLocation::from_raw(base::InputFileLocation::InputPhotoFileLocation(Box::new(location)));
        Photo {
            file_id: String::new(),
            file_unique_id: String::new(),
            id,
            width: 1,
            height: 1,
            file_size: 1,
            date: 0,
            thumbs: Vec::new(),
            location,
        }
    }

    fn input_photo(id: i64) -> base::InputPhoto {
        base::InputPhoto::InputPhoto(Box::new(types::InputPhoto { id, access_hash: 1, file_reference: vec![] }))
    }

    #[test]
    fn media_blocks_register_their_files() {
        let first = RichBlock::photo(&photo(5)).caption("cat");
        let again = RichBlock::photo(&photo(5)).spoiler();
        let nested = RichBlock::list([RichListItem::blocks([RichBlock::photo(&photo(6))])]);
        let message = InputRichMessage::blocks([RichBlock::collage([first, again]), nested])
            .media(InputRichMessageMedia::photo("pic", &photo(7)))
            .media(InputRichMessageMedia::photo("again", &photo(5)));
        let base::InputRichMessage::InputRichMessage(m) = message.build(Vec::new()) else {
            panic!("expected block content");
        };
        assert_eq!(m.photos, Some(vec![input_photo(5), input_photo(6), input_photo(7)]));
        assert_eq!((m.documents, m.users), (None, None));
        let base::PageBlock::PageBlockCollage(collage) = &m.blocks[0] else { panic!("expected a collage") };
        let base::PageBlock::PageBlockPhoto(p) = &collage.items[1] else { panic!("expected a photo") };
        assert_eq!((p.photo_id, p.spoiler), (5, true));

        let html = InputRichMessage::html("<img src=\"pic\">").media(InputRichMessageMedia::photo("pic", &photo(5)));
        let base::InputRichMessage::InputRichMessageHTML(m) = html.build(Vec::new()) else { panic!("expected html") };
        let file = types::InputRichFilePhoto { id: "pic".into(), photo: input_photo(5) };
        assert_eq!(m.files, Some(vec![base::InputRichFile::InputRichFilePhoto(Box::new(file))]));
    }

    #[tokio::test]
    async fn mentions_fill_users() {
        let client = Client::offline();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 7 }));
        client.0.peers.lock().unwrap().insert(42, peer);
        let text = Text::new().plain("hi ").bold(Text::new().mention("Bob", 42)).plain(" and ").mention("Bob", 42);
        let message = InputRichMessage::blocks([RichBlock::details("x", [RichBlock::paragraph(text)])]);
        let base::InputRichMessage::InputRichMessage(m) = message.write(&client).await.unwrap() else {
            panic!("expected block content");
        };
        let user = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 42, access_hash: 7 }));
        assert_eq!(m.users, Some(vec![user]));
    }

    #[test]
    fn flag_setters_round_trip_their_fields() {
        let on = InputRichMessage::markdown("x").rtl(true).no_autolink(true);
        let InputRichMessage::Markdown { rtl, no_autolink, .. } = &on else { panic!("expected markdown") };
        assert!(*rtl && *no_autolink);
        assert_eq!(on.rtl(false).no_autolink(false), InputRichMessage::markdown("x"));
    }
}
