use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{MessageText, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMessageDraft<'a> {
    pub(super) client: &'a Client,
    pub(super) chat_id: PeerRef,
    pub(super) draft_id: i64,
    pub(super) text: MessageText,
    pub(super) options: DraftOptions,
}

#[derive(Clone, Copy, Default)]
pub(super) struct DraftOptions {
    pub(super) parse_mode: Option<ParseMode>,
    pub(super) message_thread_id: Option<i32>,
    pub(super) can_stop: bool,
    pub(super) keep_on_stop: bool,
}

macro_rules! draft_setters {
    ($builder:ident) => {
        impl $builder<'_> {
            pub fn html(self) -> Self {
                self.parse_mode($crate::enums::ParseMode::Html)
            }

            pub fn markdown(self) -> Self {
                self.parse_mode($crate::enums::ParseMode::Markdown)
            }

            pub fn parse_mode(mut self, mode: $crate::enums::ParseMode) -> Self {
                self.options.parse_mode = Some(mode);
                self
            }

            pub fn message_thread_id(mut self, message_thread_id: i32) -> Self {
                self.options.message_thread_id = Some(message_thread_id);
                self
            }

            pub fn can_stop(mut self) -> Self {
                self.options.can_stop = true;
                self
            }

            pub fn keep_on_stop(mut self) -> Self {
                self.options.keep_on_stop = true;
                self
            }
        }
    };
}
pub(super) use draft_setters;

impl Client {
    /// Streams a partial message to the user while it is being generated; resend with the same
    /// `draft_id` to update it. Empty text is allowed.
    pub fn send_message_draft(
        &self,
        chat_id: impl Into<PeerRef>,
        draft_id: i64,
        text: impl Into<MessageText>,
    ) -> SendMessageDraft<'_> {
        SendMessageDraft {
            client: self,
            chat_id: chat_id.into(),
            draft_id,
            text: text.into(),
            options: DraftOptions::default(),
        }
    }
}

impl SendMessageDraft<'_> {
    async fn send(self) -> Result<bool> {
        let client = self.client;
        let options = self.options;
        let peer = client.resolve_peer(self.chat_id).await?;
        let (can_stop, keep_on_stop, random_id) = (options.can_stop, options.keep_on_stop, self.draft_id);
        let action = match self.text.write(client, options.parse_mode).await? {
            (_, _, Some(rich_message)) => {
                let action =
                    types::InputSendMessageRichMessageDraftAction { can_stop, keep_on_stop, random_id, rich_message };
                base::SendMessageAction::InputSendMessageRichMessageDraftAction(Box::new(action))
            }
            (text, entities, None) => {
                let text =
                    types::TextWithEntities { text: text.unwrap_or_default(), entities: entities.unwrap_or_default() };
                let text = base::TextWithEntities::TextWithEntities(Box::new(text));
                let action = types::SendMessageTextDraftAction { can_stop, keep_on_stop, random_id, text };
                base::SendMessageAction::SendMessageTextDraftAction(Box::new(action))
            }
        };
        let request = functions::messages::SetTyping { peer, top_msg_id: options.message_thread_id, action };
        client.invoke(&request).await
    }
}

draft_setters!(SendMessageDraft);
into_future!(SendMessageDraft, bool);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::types::InputRichMessage;

    #[tokio::test]
    async fn rich_drafts_send_the_rich_action() {
        let (client, mut rx) = Client::recording();
        let draft = client.send_message_draft("me", 42, InputRichMessage::html("<b>…</b>"));
        assert!(draft.message_thread_id(3).can_stop().await.is_err());
        let body = rx.try_recv().expect("no request sent");
        let html =
            types::InputRichMessageHTML { rtl: false, noautolink: false, html: "<b>…</b>".into(), files: None };
        let action = types::InputSendMessageRichMessageDraftAction {
            can_stop: true,
            keep_on_stop: false,
            random_id: 42,
            rich_message: base::InputRichMessage::InputRichMessageHTML(Box::new(html)),
        };
        let action = base::SendMessageAction::InputSendMessageRichMessageDraftAction(Box::new(action));
        assert!(body.ends_with(&[3i32.to_bytes(), action.to_bytes()].concat()));
    }
}
