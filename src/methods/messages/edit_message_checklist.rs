use super::edit_message_text::edit_request;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputChecklist, Message, PeerRef, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessageChecklist<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    checklist: InputChecklist,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    /// Replaces the whole checklist; `InputChecklist::from(checklist)` keeps the formatting of an existing one.
    pub fn edit_message_checklist(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        checklist: InputChecklist,
    ) -> EditMessageChecklist<'_> {
        EditMessageChecklist {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            checklist,
            reply_markup: None,
            business_connection_id: None,
        }
    }
}

impl EditMessageChecklist<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let request = functions::messages::EditMessage {
            media: Some(self.checklist.write(client).await?),
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            ..edit_request(peer, self.message_id)
        };
        let (updates, _) = client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Message::from_updates(client, updates, chat_id, (String::new(), None))
    }
}

into_future!(EditMessageChecklist, Message);

#[cfg(test)]
mod tests {
    use super::super::edit_message_text::edit_request;
    use crate::Client;
    use crate::raw::{Serializable, base, functions};
    use crate::types::InputChecklist;

    #[tokio::test]
    async fn replaces_the_checklist_media() {
        let (client, mut rx) = Client::recording();
        let checklist = InputChecklist::new("Trip").task(1, "Tickets").others_can_mark_tasks_as_done();
        assert!(client.edit_message_checklist("me", 5, checklist.clone()).await.is_err());
        let body = rx.try_recv().expect("no request sent");
        let expected = functions::messages::EditMessage {
            media: Some(checklist.write(&client).await.unwrap()),
            ..edit_request(base::InputPeer::InputPeerSelf, 5)
        };
        assert_eq!(body, expected.to_bytes());
        let base::InputMedia::InputMediaTodo(media) = expected.media.unwrap() else { panic!("not a checklist") };
        let base::TodoList::TodoList(todo) = media.todo;
        assert_eq!((todo.others_can_append, todo.others_can_complete, todo.list.len()), (false, true, 1));
    }
}
