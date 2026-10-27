use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<base::Updates> {
        self.send_text(chat_id, text, None).await
    }

    pub(crate) async fn send_text(&self, chat_id: i64, text: &str, reply_to: Option<i32>) -> Result<base::Updates> {
        let peer = self.resolve_peer(chat_id).await?;
        let reply_to = reply_to.map(|reply_to_msg_id| {
            base::InputReplyTo::InputReplyToMessage(Box::new(types::InputReplyToMessage {
                reply_to_msg_id,
                top_msg_id: None,
                reply_to_peer_id: None,
                quote_text: None,
                quote_entities: None,
                quote_offset: None,
                monoforum_peer_id: None,
                todo_item_id: None,
                poll_option: None,
                quote_rich: None,
            }))
        });
        let request = functions::messages::SendMessage {
            no_webpage: false,
            silent: false,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            peer,
            reply_to,
            message: text.into(),
            // Telegram drops a second message with the same random_id, so retries can't double-send.
            random_id: i64::from_le_bytes(random()),
            reply_markup: None,
            entities: None,
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            quick_reply_shortcut: None,
            effect: None,
            allow_paid_stars: None,
            suggested_post: None,
            rich_message: None,
        };
        self.invoke(&request).await
    }
}
