use crate::raw::base;
use crate::types::{Media, Photo};

/// A link preview shown under a text message.
#[derive(Debug, Clone)]
pub struct WebPage {
    pub id: i64,
    pub url: String,
    /// The url without the scheme, as shown in the preview.
    pub display_url: String,
    /// The page type, such as `article`, `photo` or `video`.
    pub kind: Option<String>,
    pub site_name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub photo: Option<Photo>,
    /// The page's main document: a video, audio, animation, sticker or file.
    pub media: Option<Media>,
    pub embed_url: Option<String>,
    pub embed_type: Option<String>,
    pub embed_width: Option<i32>,
    pub embed_height: Option<i32>,
    pub duration: Option<i32>,
    pub author: Option<String>,
    /// The preview can show its media large.
    pub has_large_media: bool,
    /// The sender asked for the media to be shown large.
    pub prefer_large_media: bool,
    /// The sender asked for the media to be shown small.
    pub prefer_small_media: bool,
    /// The sender picked the preview, rather than Telegram taking the first link.
    pub manual: bool,
    /// The url was checked and is safe to open without a warning.
    pub safe: bool,
}

impl WebPage {
    /// Only a loaded page: a pending, empty or unchanged one gives `None`.
    pub(crate) fn parse(media: &base::MessageMedia) -> Option<WebPage> {
        let base::MessageMedia::MessageMediaWebPage(m) = media else { return None };
        let base::WebPage::WebPage(page) = &m.webpage else { return None };
        Some(WebPage {
            id: page.id,
            url: page.url.clone(),
            display_url: page.display_url.clone(),
            kind: page.r#type.clone(),
            site_name: page.site_name.clone(),
            title: page.title.clone(),
            description: page.description.clone(),
            photo: page.photo.as_ref().and_then(Photo::parse),
            media: page.document.as_ref().and_then(Media::from_document),
            embed_url: page.embed_url.clone(),
            embed_type: page.embed_type.clone(),
            embed_width: page.embed_width,
            embed_height: page.embed_height,
            duration: page.duration,
            author: page.author.clone(),
            has_large_media: page.has_large_media,
            prefer_large_media: m.force_large_media,
            prefer_small_media: m.force_small_media,
            manual: m.manual,
            safe: m.safe,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::WebPage;
    use crate::enums::MediaKind;
    use crate::raw::{base, types};

    pub(crate) fn web_page_media(webpage: base::WebPage) -> base::MessageMedia {
        base::MessageMedia::MessageMediaWebPage(Box::new(types::MessageMediaWebPage {
            force_large_media: true,
            force_small_media: false,
            manual: true,
            safe: false,
            webpage,
        }))
    }

    pub(crate) fn page(url: &str) -> types::WebPage {
        types::WebPage {
            has_large_media: false,
            video_cover_photo: false,
            id: 5,
            url: url.into(),
            display_url: url.trim_start_matches("https://").into(),
            hash: 0,
            r#type: Some("video".into()),
            site_name: Some("Example".into()),
            title: Some("A clip".into()),
            description: None,
            photo: None,
            embed_url: None,
            embed_type: None,
            embed_width: None,
            embed_height: None,
            duration: Some(8),
            author: None,
            document: None,
            cached_page: None,
            attributes: None,
        }
    }

    #[test]
    fn reads_a_full_page_with_its_video() {
        let video = base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
            round_message: false,
            supports_streaming: true,
            nosound: false,
            duration: 8.0,
            w: 640,
            h: 360,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }));
        let base::MessageMedia::MessageMediaDocument(document) =
            super::super::media::tests::document_media("video/mp4", vec![video])
        else {
            panic!()
        };
        let raw =
            types::WebPage { has_large_media: true, document: document.document, ..page("https://example.com/v") };
        let page = WebPage::parse(&web_page_media(base::WebPage::WebPage(Box::new(raw)))).expect("a full page");
        assert_eq!((page.url.as_str(), page.display_url.as_str()), ("https://example.com/v", "example.com/v"));
        assert_eq!(
            (page.kind.as_deref(), page.site_name.as_deref(), page.duration),
            (Some("video"), Some("Example"), Some(8))
        );
        assert_eq!(page.media.map(|m| m.kind()), Some(MediaKind::Video));
        assert!(
            page.has_large_media && page.prefer_large_media && !page.prefer_small_media && page.manual && !page.safe
        );
    }

    #[test]
    fn pending_empty_and_unchanged_pages_give_none() {
        let pending = types::WebPagePending { id: 1, url: Some("https://a.b".into()), date: 0 };
        let pages = [
            base::WebPage::WebPagePending(Box::new(pending)),
            base::WebPage::WebPageEmpty(Box::new(types::WebPageEmpty { id: 1, url: None })),
            base::WebPage::WebPageNotModified(types::WebPageNotModified { cached_page_views: None }),
        ];
        for webpage in pages {
            assert!(WebPage::parse(&web_page_media(webpage)).is_none());
        }
        assert!(WebPage::parse(&base::MessageMedia::MessageMediaEmpty).is_none());
    }
}
