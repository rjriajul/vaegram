use std::sync::PoisonError;

use crate::Client;
use crate::types::LinkPreviewOptions;

impl Client {
    /// Changes the link preview options every later message sent or edited without its own
    /// `.link_preview_options(..)` uses. `None` drops the default. Its `url` is ignored: one link for every message
    /// makes no sense.
    ///
    /// Calls already in flight keep the options they started with.
    pub fn set_link_preview_options(&self, options: Option<LinkPreviewOptions>) {
        *self.0.link_preview_options.lock().unwrap_or_else(PoisonError::into_inner) = options;
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::types::LinkPreviewOptions;

    #[test]
    fn set_link_preview_options_changes_the_default() {
        let client = Client::offline();
        assert_eq!(client.link_preview(None), None);
        client.set_link_preview_options(Some(LinkPreviewOptions::new().url("https://a.b").show_above_text(true)));
        assert_eq!(client.link_preview(None), Some(LinkPreviewOptions::new().show_above_text(true)));
        let own = LinkPreviewOptions::new().is_disabled(true);
        assert_eq!(client.link_preview(Some(&own)), Some(own));
        client.set_link_preview_options(None);
        assert_eq!(client.link_preview(None), None);
    }
}
