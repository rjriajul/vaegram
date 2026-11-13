use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineMessageId, InlineQueryResult, SentGuestMessage};

/// Request built by [`Client::answer_guest_query`]. Awaiting it returns the [`SentGuestMessage`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AnswerGuestQuery<'a> {
    client: &'a Client,
    guest_query_id: i64,
    result: InlineQueryResult,
}

impl Client {
    /// Answers a guest query with one inline result.
    ///
    /// Awaiting the request returns a [`SentGuestMessage`] holding the inline message id of the message the bot sent,
    /// which [`SentGuestMessage::edit_text`] and [`SentGuestMessage::edit_reply_markup`] edit.
    pub fn answer_guest_query(
        &self,
        guest_query_id: i64,
        result: impl Into<InlineQueryResult>,
    ) -> AnswerGuestQuery<'_> {
        AnswerGuestQuery { client: self, guest_query_id, result: result.into() }
    }
}

impl AnswerGuestQuery<'_> {
    async fn send(self) -> Result<SentGuestMessage> {
        let result = self.result.write(self.client).await?;
        let request = functions::messages::SetBotGuestChatResult { query_id: self.guest_query_id, result };
        let id = InlineMessageId(self.client.invoke(&request).await?);
        Ok(SentGuestMessage::new(self.client.clone(), id))
    }
}

into_future!(AnswerGuestQuery, SentGuestMessage);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::InlineQueryResultArticle;

    #[tokio::test]
    async fn sends_the_result_on_await() {
        let (client, mut rx) = Client::recording();
        let answer = client.answer_guest_query(9, InlineQueryResultArticle::new("t", "hi"));
        assert!(rx.try_recv().is_err(), "nothing is sent before the await");
        let _ = answer.await;
        let body = rx.try_recv().expect("no request sent");
        let id = u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(id, functions::messages::SetBotGuestChatResult::CONSTRUCTOR_ID);
        assert_eq!(i64::from_le_bytes(body[4..12].try_into().unwrap()), 9);
    }

    #[tokio::test]
    async fn the_answer_is_edited_by_the_id_telegram_returned() {
        let id = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        let id = base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id));
        let (client, mut rx) = Client::scripted(vec![Ok(id.to_bytes())]);
        let sent = client.answer_guest_query(9, InlineQueryResultArticle::new("t", "hi")).await.unwrap();
        let _ = rx.try_recv().expect("no answer sent");
        let _ = sent.edit_text("edited").await;
        let edit = functions::messages::EditInlineBotMessage {
            no_webpage: false,
            invert_media: false,
            id,
            message: Some("edited".into()),
            media: None,
            reply_markup: None,
            entities: None,
            rich_message: None,
        };
        assert_eq!(rx.try_recv().expect("no edit sent"), edit.to_bytes());
    }
}
