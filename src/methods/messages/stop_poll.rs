use super::edit_message_text::edit_request;
use super::send_poll::poll;
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef, ReplyMarkup};

/// Request built by [`Client::stop_poll`].
#[must_use = "a request does nothing until it is awaited"]
pub struct StopPoll<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    /// Closes a poll.
    ///
    /// Awaiting the request returns the edited [`Message`], with the final results in [`Message::poll`]. Outside a
    /// business chat it reads the message first, and a message id without a poll fails with `Error::InvalidArgument`.
    pub fn stop_poll(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> StopPoll<'_> {
        let chat_id = chat_id.into();
        StopPoll { client: self, chat_id, message_id, reply_markup: None, business_connection_id: None }
    }
}

impl StopPoll<'_> {
    /// Replaces the inline keyboard of the message.
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    /// Closes a poll sent on behalf of the business account behind this connection id.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let chat_id = client.peer_id(self.chat_id).await?;
        let peer = client.resolve_peer(chat_id).await?;
        // Outside a business chat Telegram closes a poll by its id, which only the message carries. A business
        // chat's messages can't be read through the connection, and closing there takes no id.
        let read = match self.business_connection_id {
            Some(_) => None,
            None => {
                let (found, entities) = client.get_raw_messages(chat_id, &[self.message_id]).await?;
                let message = found
                    .into_iter()
                    .flatten()
                    .find(|m| matches!(m.media, Some(base::MessageMedia::MessageMediaPoll(_))))
                    .ok_or(Error::InvalidArgument("no poll at that message id"))?;
                Some((message, entities))
            }
        };
        let poll_id = match read.as_ref().and_then(|(m, _)| m.media.as_ref()) {
            Some(base::MessageMedia::MessageMediaPoll(media)) => {
                let base::Poll::Poll(poll) = &media.poll;
                poll.id
            }
            _ => 0,
        };

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
            reply_markup: ReplyMarkup::write_option(self.reply_markup.as_ref(), self.client).await?,
            ..edit_request(peer, self.message_id)
        };
        let (updates, _) = client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        let Some((mut message, entities)) = read else {
            return Message::from_updates(client, updates, chat_id, (String::new(), None));
        };
        // Telegram may answer with only the poll's new state and no edited message, so the message read above
        // takes that state.
        let state = poll_update(&updates).map(|u| (u.poll.clone(), u.results.clone()));
        if let Some(edited) = Message::all_from_updates(client, updates).next() {
            return Ok(edited);
        }
        if let Some(base::MessageMedia::MessageMediaPoll(media)) = &mut message.media {
            if let Some((poll, results)) = state {
                if let Some(poll) = poll {
                    media.poll = poll;
                }
                media.results = results;
            }
            // The edit went through, so the poll is closed even when the update left the poll out.
            let base::Poll::Poll(poll) = &mut media.poll;
            poll.closed = true;
        }
        Ok(Message::parse(client, message, &entities))
    }
}

/// The first poll state in `updates`.
fn poll_update(updates: &base::Updates) -> Option<&types::UpdateMessagePoll> {
    let updates = match updates {
        base::Updates::Updates(u) => &u.updates,
        base::Updates::UpdatesCombined(u) => &u.updates,
        base::Updates::UpdateShort(u) => std::slice::from_ref(&u.update),
        _ => return None,
    };
    updates.iter().find_map(|u| match u {
        base::Update::UpdateMessagePoll(u) => Some(&**u),
        _ => None,
    })
}

into_future!(StopPoll, Message);

impl crate::methods::OnBehalf for StopPoll<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_poll::{poll, text_with_entities};
    use crate::Client;
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    fn setup(replies: Vec<Vec<u8>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies.into_iter().map(Ok).collect());
        let user = types::InputPeerUser { user_id: 7, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, rx)
    }

    fn quiz(closed: bool) -> base::Poll {
        let answer = |option: &[u8]| {
            let text = text_with_entities(String::from_utf8_lossy(option).into_owned());
            let answer = types::PollAnswer { text, option: option.to_vec(), media: None, added_by: None, date: None };
            base::PollAnswer::PollAnswer(Box::new(answer))
        };
        let quiz = types::Poll { quiz: true, closed, ..poll(9, "2 + 2?".into(), vec![answer(b"3"), answer(b"4")]) };
        base::Poll::Poll(Box::new(quiz))
    }

    fn results(voters: i32) -> base::PollResults {
        let results = types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: None,
            total_voters: Some(voters),
            recent_voters: None,
            solution: None,
            solution_entities: None,
            solution_media: None,
        };
        base::PollResults::PollResults(Box::new(results))
    }

    // Message 5 in the chat with user 7, with an open quiz nobody voted in yet.
    fn found() -> Vec<u8> {
        let mut raw = Message::bare(5, get_peer(7), 0);
        let media = types::MessageMediaPoll { poll: quiz(false), results: results(0), attached_media: None };
        raw.media = Some(base::MessageMedia::MessageMediaPoll(Box::new(media)));
        let found = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(raw))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Messages::Messages(Box::new(found)).to_bytes()
    }

    // How Telegram answers the edit that closes a poll: the poll's new state, without the edited message.
    fn closed(poll: Option<base::Poll>) -> Vec<u8> {
        let update = types::UpdateMessagePoll {
            peer: Some(get_peer(7)),
            msg_id: Some(5),
            top_msg_id: None,
            poll_id: 9,
            poll,
            results: results(4),
        };
        let updates = types::Updates {
            updates: vec![base::Update::UpdateMessagePoll(Box::new(update))],
            users: Vec::new(),
            chats: Vec::new(),
            date: 1,
            seq: 0,
        };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    #[tokio::test]
    async fn an_answer_with_only_the_poll_state_returns_the_closed_poll() {
        let (client, mut rx) = setup(vec![found(), closed(Some(quiz(true)))]);
        let message = client.stop_poll(7, 5).await.unwrap();
        let poll = message.poll().expect("the message lost its poll");
        assert_eq!(
            (message.id, poll.question.as_str(), poll.is_closed, poll.total_voter_count),
            (5, "2 + 2?", true, 4)
        );
        assert_eq!(rx.try_recv().unwrap()[..4], messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes());
        assert_eq!(rx.try_recv().unwrap()[..4], messages::EditMessage::CONSTRUCTOR_ID.to_le_bytes());
    }

    #[tokio::test]
    async fn a_state_without_the_poll_still_marks_it_closed() {
        let (client, _rx) = setup(vec![found(), closed(None)]);
        let message = client.stop_poll(7, 5).await.unwrap();
        let poll = message.poll().expect("the message lost its poll");
        assert_eq!((poll.question.as_str(), poll.is_closed, poll.total_voter_count), ("2 + 2?", true, 4));
    }
}
