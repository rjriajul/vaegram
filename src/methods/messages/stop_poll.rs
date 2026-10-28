use super::edit_message_text::edit_request;
use super::send_poll::poll;
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct StopPoll<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    /// Closes a poll and returns the edited message; the final results are in `raw.media`.
    pub fn stop_poll(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> StopPoll<'_> {
        StopPoll { client: self, chat_id: chat_id.into(), message_id, reply_markup: None }
    }
}

impl StopPoll<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let chat_id = client.peer_id(self.chat_id).await?;
        let peer = client.resolve_peer(chat_id).await?;
        // Telegram closes a poll by its id, which only the message carries.
        let (found, _) = client.get_raw_messages(chat_id, &[self.message_id]).await?;
        let poll_id = found
            .into_iter()
            .flatten()
            .find_map(|m| match m.media {
                Some(base::MessageMedia::MessageMediaPoll(media)) => {
                    let base::Poll::Poll(poll) = &media.poll;
                    Some(poll.id)
                }
                _ => None,
            })
            .ok_or(Error::InvalidArgument("no poll at that message id"))?;

        let closed = types::Poll { closed: true, ..poll(poll_id, String::new(), Vec::new()) };
        let media = types::InputMediaPoll {
            poll: base::Poll::Poll(Box::new(closed)),
            correct_answers: None,
            attached_media: None,
            solution: None,
            solution_entities: None,
            solution_media: None,
        };
        let request = functions::messages::EditMessage {
            media: Some(base::InputMedia::InputMediaPoll(Box::new(media))),
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            ..edit_request(peer, self.message_id)
        };
        let updates = client.invoke(&request).await?;
        Message::from_updates(client, updates, chat_id, (String::new(), None))
    }
}

into_future!(StopPoll, Message);
