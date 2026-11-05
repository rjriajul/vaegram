use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{RemoteCall, functions};
use crate::types::{InlineMessageId, LinkPreviewOptions, MessageText, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::edit_inline_text`]. Awaiting it returns Telegram's `bool` answer.
pub struct EditInlineText<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    text: MessageText,
    parse_mode: Option<ParseMode>,
    reply_markup: Option<ReplyMarkup>,
    no_webpage: bool,
    show_caption_above_media: bool,
    link_preview_options: Option<LinkPreviewOptions>,
    business_connection_id: Option<String>,
    apply_client_default: bool,
}

impl Client {
    /// Replaces the text of an inline message.
    ///
    /// The text is parsed with the client's default parse mode unless [`html`](EditInlineText::html),
    /// [`markdown`](EditInlineText::markdown) or [`parse_mode`](EditInlineText::parse_mode) is set. The client's link
    /// preview default applies unless [`link_preview_options`](EditInlineText::link_preview_options) is set.
    pub fn edit_inline_text(
        &self,
        inline_message_id: InlineMessageId,
        text: impl Into<MessageText>,
    ) -> EditInlineText<'_> {
        EditInlineText {
            client: self,
            inline_message_id,
            text: text.into(),
            parse_mode: None,
            reply_markup: None,
            no_webpage: false,
            show_caption_above_media: false,
            link_preview_options: None,
            business_connection_id: None,
            apply_client_default: true,
        }
    }

    /// Sends a request about an inline message, on behalf of a business when `business_connection_id` is given.
    pub(super) async fn invoke_inline_on_behalf<R: RemoteCall>(
        &self,
        dc_id: i32,
        business_connection_id: Option<String>,
        request: R,
    ) -> Result<R::Return> {
        match business_connection_id {
            Some(connection_id) => {
                self.invoke_inline(dc_id, functions::InvokeWithBusinessConnection { connection_id, query: request })
                    .await
            }
            None => self.invoke_inline(dc_id, request).await,
        }
    }
}

impl EditInlineText<'_> {
    /// Attaches a reply markup to the edited message.
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    /// Sends the text without a link preview.
    pub fn disable_web_page_preview(mut self) -> Self {
        self.no_webpage = true;
        self
    }

    /// Shows the caption above the media, or the link preview above the text.
    pub fn show_caption_above_media(mut self) -> Self {
        self.show_caption_above_media = true;
        self
    }

    /// Which link to preview, how, and whether to show it above the text. Replaces the client default.
    pub fn link_preview_options(mut self, options: LinkPreviewOptions) -> Self {
        self.link_preview_options = Some(options);
        self
    }

    /// Edits a message sent through a business connection.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    /// For caption edits: the client's link preview default is meant for text, not for moving captions.
    pub(super) fn without_client_default(mut self) -> Self {
        self.apply_client_default = false;
        self
    }

    async fn send(self) -> Result<bool> {
        let client = self.client;
        let preview = if self.apply_client_default {
            client.link_preview(self.link_preview_options.as_ref())
        } else {
            self.link_preview_options.clone()
        }
        .map(|o| o.resolve())
        .unwrap_or_default();
        let no_webpage = self.no_webpage || preview.no_webpage;
        let (message, entities, rich_message) = self.text.write(client, self.parse_mode).await?;
        let dc_id = self.inline_message_id.dc_id();
        let request = functions::messages::EditInlineBotMessage {
            no_webpage,
            invert_media: self.show_caption_above_media || preview.invert_media,
            id: self.inline_message_id.0,
            message,
            media: if no_webpage { None } else { preview.media },
            reply_markup: ReplyMarkup::write_option(self.reply_markup.as_ref(), self.client).await?,
            entities,
            rich_message,
        };
        client.invoke_inline_on_behalf(dc_id, self.business_connection_id, request).await
    }
}

caption_setters!(EditInlineText);
into_future!(EditInlineText, bool);

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::raw::{Identifiable, functions};
    use crate::types::InlineMessageId;

    pub(in crate::methods::messages) fn id() -> InlineMessageId {
        let id = crate::raw::types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        InlineMessageId(crate::raw::base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id)))
    }

    #[tokio::test]
    async fn inline_edit_on_behalf_wraps_business() {
        let (client, mut rx) = Client::recording();
        let _ = client.edit_inline_text(id(), "hi").business_connection_id("conn").await;
        let body = rx.try_recv().unwrap();
        assert_eq!(
            u32::from_le_bytes(body[..4].try_into().unwrap()),
            <functions::InvokeWithBusinessConnection<functions::messages::EditInlineBotMessage> as Identifiable>::CONSTRUCTOR_ID
        );
        // The connection id is a TL string: length byte, then the bytes.
        assert!(body.windows(5).any(|w| w == [4, b'c', b'o', b'n', b'n']), "the connection id isn't in the request");
    }
}
