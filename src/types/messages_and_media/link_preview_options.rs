use crate::raw::base;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPreviewOptions {
    pub url: Option<String>,
    pub prefer_small_media: bool,
    pub prefer_large_media: bool,
    pub show_above_text: bool,
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
            prefer_small_media: media.force_small_media,
            prefer_large_media: media.force_large_media,
            show_above_text: invert_media,
        })
    }
}
