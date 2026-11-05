use crate::Client;
use crate::raw::{base, types};

/// What to preview for a link in a message. Read from received messages, and passed to `send_message`,
/// `edit_message_text` and `edit_inline_text` through `.link_preview_options(..)`.
/// `ClientBuilder::link_preview_options` sets a default for every call that gives none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkPreviewOptions {
    /// The link to preview instead of the first one in the text.
    pub url: Option<String>,
    /// Send without any preview. Wins over `url`. Never set on received messages.
    pub is_disabled: bool,
    /// Shrinks the preview's media.
    pub prefer_small_media: bool,
    /// Enlarges the preview's media.
    pub prefer_large_media: bool,
    /// Puts the preview above the message text instead of below it.
    pub show_above_text: bool,
}

/// How a send or edit request expresses link preview options.
#[derive(Debug, Default)]
pub(crate) struct Preview {
    pub(crate) no_webpage: bool,
    pub(crate) invert_media: bool,
    pub(crate) media: Option<base::InputMedia>,
}

impl LinkPreviewOptions {
    pub(crate) fn parse(media: Option<&base::MessageMedia>, invert_media: bool) -> Option<LinkPreviewOptions> {
        let Some(base::MessageMedia::MessageMediaWebPage(media)) = media else { return None };
        let url = match &media.webpage {
            base::WebPage::WebPage(w) => Some(w.url.clone()),
            base::WebPage::WebPageEmpty(w) => w.url.clone(),
            base::WebPage::WebPagePending(w) => w.url.clone(),
            base::WebPage::WebPageNotModified(_) => None,
        };
        Some(LinkPreviewOptions {
            url,
            is_disabled: false,
            prefer_small_media: media.force_small_media,
            prefer_large_media: media.force_large_media,
            show_above_text: invert_media,
        })
    }

    /// `no_webpage` when disabled; otherwise a preview of `url` as optional web page media.
    pub(crate) fn resolve(&self) -> Preview {
        let media = match (&self.url, self.is_disabled) {
            (Some(url), false) => {
                let page = types::InputMediaWebPage {
                    force_large_media: self.prefer_large_media,
                    force_small_media: self.prefer_small_media,
                    optional: true,
                    url: url.clone(),
                };
                Some(base::InputMedia::InputMediaWebPage(Box::new(page)))
            }
            _ => None,
        };
        Preview { no_webpage: self.is_disabled, invert_media: self.show_above_text, media }
    }
}

impl Client {
    /// The options a request uses: its own, or the client default without its `url` (one link for every message
    /// makes no sense).
    pub(crate) fn link_preview(&self, own: Option<&LinkPreviewOptions>) -> Option<LinkPreviewOptions> {
        own.cloned()
            .or_else(|| self.0.link_preview_options.as_ref().map(|d| LinkPreviewOptions { url: None, ..d.clone() }))
    }
}

#[cfg(test)]
mod tests {
    use super::LinkPreviewOptions;
    use crate::raw::base;

    fn options(url: Option<&str>) -> LinkPreviewOptions {
        LinkPreviewOptions { url: url.map(Into::into), ..Default::default() }
    }

    #[test]
    fn url_resolves_to_optional_web_page_media() {
        let preview = LinkPreviewOptions { prefer_large_media: true, ..options(Some("https://a.b")) }.resolve();
        let Some(base::InputMedia::InputMediaWebPage(page)) = preview.media else { panic!("no web page media") };
        assert_eq!(
            (page.url.as_str(), page.optional, page.force_large_media, page.force_small_media),
            ("https://a.b", true, true, false)
        );
        assert!(!preview.no_webpage && !preview.invert_media);
    }

    #[test]
    fn disabled_ignores_url_and_sets_no_webpage() {
        let preview = LinkPreviewOptions { is_disabled: true, ..options(Some("https://a.b")) }.resolve();
        assert!(preview.no_webpage && preview.media.is_none());
    }

    #[test]
    fn above_text_inverts_media() {
        let preview = LinkPreviewOptions { show_above_text: true, ..options(None) }.resolve();
        assert!(preview.invert_media && preview.media.is_none() && !preview.no_webpage);
    }
}
