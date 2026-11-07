use crate::Client;
use crate::methods::SendMessageDraft;
use crate::types::{InputRichMessage, PeerRef};

impl Client {
    /// Streams a partial [`InputRichMessage`] to the user while it is being generated.
    ///
    /// Sending again with the same `draft_id` updates the draft. Same as [`Client::send_message_draft`] with a rich
    /// message; the parse mode setters of the request don't apply to it.
    pub fn send_rich_message_draft(
        &self,
        chat_id: impl Into<PeerRef>,
        draft_id: i64,
        rich_message: InputRichMessage,
    ) -> SendMessageDraft<'_> {
        self.send_message_draft(chat_id, draft_id, rich_message)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::InputRichMessage;

    #[tokio::test]
    async fn sends_the_rich_draft_action() {
        let (client, mut rx) = Client::recording();
        let draft = client.send_rich_message_draft("me", 42, InputRichMessage::html("<b>…</b>")).keep_on_stop();
        assert!(draft.await.is_err());
        let html =
            types::InputRichMessageHTML { rtl: false, noautolink: false, html: "<b>…</b>".into(), files: None };
        let action = types::InputSendMessageRichMessageDraftAction {
            can_stop: false,
            keep_on_stop: true,
            random_id: 42,
            rich_message: base::InputRichMessage::InputRichMessageHTML(Box::new(html)),
        };
        let request = functions::messages::SetTyping {
            peer: base::InputPeer::InputPeerSelf,
            top_msg_id: None,
            action: base::SendMessageAction::InputSendMessageRichMessageDraftAction(Box::new(action)),
        };
        assert_eq!(rx.try_recv().expect("no request sent"), request.to_bytes());
    }
}
