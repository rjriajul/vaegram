use super::Listen;
use crate::Client;
use crate::types::{CallbackQuery, PeerRef};

impl Client {
    /// Waits for the next button press in `chat_id`: `listen::<CallbackQuery>().chat(chat_id)`, with the same rules.
    /// Pin it to one keyboard with `message_id`. Presses on inline messages have no chat, so they never match here.
    pub fn wait_for_callback_query(&self, chat_id: impl Into<PeerRef>) -> Listen<'_, CallbackQuery> {
        self.listen().chat(chat_id)
    }
}

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use crate::Client;
    use crate::dispatcher::listeners::Offer;
    use crate::dispatcher::listeners::tests::{inline_query, query};
    use crate::types::CallbackQuery;

    #[tokio::test]
    async fn message_id_pins_the_button() {
        let client = Client::offline();
        let mut listen = client.wait_for_callback_query(7).message_id(2).into_future();
        tokio::select! {
            biased;
            _ = &mut listen => panic!("nothing was offered"),
            _ = std::future::ready(()) => {}
        }
        let (book, me) = (&client.0.handlers.listeners.callbacks, client.0.me.lock().unwrap().clone());
        assert!(matches!(book.offer(query(&client, 7, 7, 3), &me), Offer::Unclaimed(_)));
        assert!(matches!(book.offer(query(&client, 7, 8, 2), &me), Offer::Unclaimed(_)));
        assert!(matches!(book.offer(query(&client, 7, 7, 2), &me), Offer::Claimed));
        assert_eq!(listen.await.unwrap().message_id(), Some(2));
    }

    #[tokio::test]
    async fn inline_callbacks_need_inline_message_id() {
        let client = Client::offline();
        let (book, me) = (&client.0.handlers.listeners.callbacks, client.0.me.lock().unwrap().clone());
        let mut in_chat = client.wait_for_callback_query(7).into_future();
        let wanted = inline_query(&client, 7, 1).inline_message_id().cloned().unwrap();
        let mut inline = client.listen::<CallbackQuery>().inline_message_id(wanted).into_future();
        tokio::select! {
            biased;
            _ = &mut in_chat => panic!("nothing was offered"),
            _ = &mut inline => panic!("nothing was offered"),
            _ = std::future::ready(()) => {}
        }
        assert!(matches!(book.offer(inline_query(&client, 7, 2), &me), Offer::Unclaimed(_)));
        assert!(matches!(book.offer(inline_query(&client, 7, 1), &me), Offer::Claimed));
        assert!(inline.await.unwrap().inline_message_id().is_some());
        assert!(book.waiting(), "the chat listener took an inline press");
        drop(in_chat);
    }
}
