use super::send_message::SendMessage;
use crate::Client;
use crate::types::{LinkPreviewOptions, PeerRef, Text};

impl Client {
    /// Sends a text message that previews `url`, whatever links the text has.
    ///
    /// Calling [`SendMessage::link_preview_options`] replaces these options, so set `url` there again. Awaiting the
    /// request returns the sent [`Message`](crate::types::Message).
    pub fn send_web_page(
        &self,
        chat_id: impl Into<PeerRef>,
        text: impl Into<Text>,
        url: impl Into<String>,
    ) -> SendMessage<'_> {
        let preview = LinkPreviewOptions::new().url(url);
        self.send_message(chat_id, text).link_preview_options(preview)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::Identifiable;
    use crate::raw::functions::messages::SendMedia;
    use crate::types::LinkPreviewOptions;

    fn body_id(body: &[u8]) -> u32 {
        u32::from_le_bytes(body[..4].try_into().unwrap())
    }

    #[tokio::test]
    async fn url_sends_web_page_media() {
        let (client, mut rx) = Client::recording();
        let _ = client.send_web_page("me", "look", "https://a.b").await;
        let body = rx.try_recv().unwrap();
        assert_eq!(body_id(&body), SendMedia::CONSTRUCTOR_ID);
        let needle = {
            let mut v = <crate::raw::types::InputMediaWebPage as Identifiable>::CONSTRUCTOR_ID.to_le_bytes().to_vec();
            v.extend_from_slice(&[0b100, 0, 0, 0]);
            v
        };
        assert!(body.windows(needle.len()).any(|w| w == needle), "no optional web page media in the request");
    }

    #[tokio::test]
    async fn disabled_ignores_url_and_sends_a_plain_message() {
        let (client, mut rx) = Client::recording();
        let options = LinkPreviewOptions::new().url("https://a.b").is_disabled(true);
        let _ = client.send_web_page("me", "look", "https://a.b").link_preview_options(options).await;
        let body = rx.try_recv().unwrap();
        assert_eq!(body_id(&body), crate::raw::functions::messages::SendMessage::CONSTRUCTOR_ID);
        // no_webpage is flag bit 1.
        assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()) & 2, 2);
    }

    #[tokio::test]
    async fn the_client_default_applies_flags_but_not_its_url() {
        let default = LinkPreviewOptions::new().url("https://x.y").show_above_text(true);
        let (mut client, mut rx) = Client::recording();
        std::sync::Arc::get_mut(&mut client.0).unwrap().link_preview_options = Some(default);
        let _ = client.send_message("me", "hi").await;
        let body = rx.try_recv().unwrap();
        assert_eq!(body_id(&body), crate::raw::functions::messages::SendMessage::CONSTRUCTOR_ID);
        // invert_media is flag bit 16.
        assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()) & (1 << 16), 1 << 16);
    }
}
