use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::types::{InputChecklist, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendChecklist<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    checklist: InputChecklist,
    options: SendOptions,
}

impl Client {
    /// Bots can send checklists only on behalf of a business account, see
    /// [`SendChecklist::business_connection_id`].
    pub fn send_checklist(&self, chat_id: impl Into<PeerRef>, checklist: InputChecklist) -> SendChecklist<'_> {
        SendChecklist { client: self, chat_id: chat_id.into(), checklist, options: SendOptions::default() }
    }
}

impl SendChecklist<'_> {
    async fn send(self) -> Result<Message> {
        let media = self.checklist.write(self.client).await?;
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendChecklist);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::{InvokeWithBusinessConnection, messages};
    use crate::raw::{Identifiable, types};
    use crate::types::InputChecklist;

    #[tokio::test]
    async fn sends_through_the_business_connection() {
        let (client, mut rx) = Client::recording();
        let checklist = InputChecklist::new("Trip").task(1, "Tickets").others_can_add_tasks();
        assert!(client.send_checklist("me", checklist).business_connection_id("c1").await.is_err());
        let body = rx.try_recv().expect("no request sent");
        let id = |at: usize| u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
        assert_eq!(id(0), InvokeWithBusinessConnection::<messages::SendMedia>::CONSTRUCTOR_ID);
        assert_eq!((&body[4..7], id(8)), (&b"\x02c1"[..], messages::SendMedia::CONSTRUCTOR_ID));
        let todo = types::InputMediaTodo::CONSTRUCTOR_ID.to_le_bytes();
        assert!(body.windows(4).any(|w| w == todo));
    }
}
