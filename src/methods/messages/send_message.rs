use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<base::Updates> {
        let peer = self.resolve_peer(chat_id).await?;
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
            reply_to: None,
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
