use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMessageDraft<'a> {
    pub(super) client: &'a Client,
    pub(super) chat_id: PeerRef,
    pub(super) draft_id: i64,
    pub(super) text: String,
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
        text: impl Into<String>,
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
        let (text, entities) = client.parse_text(&self.text, options.parse_mode).await?;
        let action = types::SendMessageTextDraftAction {
            can_stop: options.can_stop,
            keep_on_stop: options.keep_on_stop,
            random_id: self.draft_id,
            text: base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities })),
        };
        let request = functions::messages::SetTyping {
            peer,
            top_msg_id: options.message_thread_id,
            action: base::SendMessageAction::SendMessageTextDraftAction(Box::new(action)),
        };
        client.invoke(&request).await
    }
}

draft_setters!(SendMessageDraft);
into_future!(SendMessageDraft, bool);
