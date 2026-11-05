use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::{Message, PeerRef, Story};

/// What a new message replies to: a message or a story, with an optional quote.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplyParameters {
    message_id: i32,
    chat_id: Option<PeerRef>,
    quote: Option<String>,
    quote_parse_mode: Option<ParseMode>,
    quote_position: Option<i32>,
    checklist_task_id: Option<i32>,
    story_id: Option<i32>,
}

impl ReplyParameters {
    /// A reply to the message `message_id`, in the chat the reply is sent to unless `chat_id` says otherwise.
    pub fn new(message_id: i32) -> Self {
        ReplyParameters {
            message_id,
            chat_id: None,
            quote: None,
            quote_parse_mode: None,
            quote_position: None,
            checklist_task_id: None,
            story_id: None,
        }
    }

    /// A reply to the story `story_id` posted by `chat_id`. A story reply takes no quote or checklist task.
    pub fn story(chat_id: impl Into<PeerRef>, story_id: i32) -> Self {
        ReplyParameters { story_id: Some(story_id), ..ReplyParameters::new(0).chat_id(chat_id) }
    }

    /// The chat of the replied message, when it is not the chat the reply is sent to.
    pub fn chat_id(mut self, chat_id: impl Into<PeerRef>) -> Self {
        self.chat_id = Some(chat_id.into());
        self
    }

    /// Quoted part of the replied message. It must match the message exactly, formatting included.
    pub fn quote(mut self, quote: impl Into<String>) -> Self {
        self.quote = Some(quote.into());
        self
    }

    /// How to parse the markup in the quote.
    pub fn quote_parse_mode(mut self, mode: ParseMode) -> Self {
        self.quote_parse_mode = Some(mode);
        self
    }

    /// Where the quote starts in the replied message's text.
    pub fn quote_position(mut self, offset: i32) -> Self {
        self.quote_position = Some(offset);
        self
    }

    /// Reply to this task of a checklist message.
    pub fn checklist_task_id(mut self, task_id: i32) -> Self {
        self.checklist_task_id = Some(task_id);
        self
    }

    pub(crate) async fn write(&self, client: &Client, chat_id: i64) -> Result<base::InputReplyTo> {
        if let Some(story_id) = self.story_id {
            if self.quote.is_some() || self.quote_position.is_some() || self.checklist_task_id.is_some() {
                return Err(Error::InvalidArgument("a story reply takes no quote or checklist task"));
            }
            let chat = self.chat_id.as_ref().ok_or(Error::InvalidArgument("a story reply needs the story's chat"))?;
            let peer = client.resolve(chat).await?.1;
            return Ok(base::InputReplyTo::InputReplyToStory(Box::new(types::InputReplyToStory { peer, story_id })));
        }
        let reply_to_peer_id = match &self.chat_id {
            Some(PeerRef::Id(id)) if *id == chat_id => None,
            Some(chat) => Some(client.resolve(chat).await?).filter(|(id, _)| *id != chat_id).map(|(_, peer)| peer),
            None => None,
        };
        let (quote_text, quote_entities) = match &self.quote {
            Some(quote) => {
                let (text, entities) = client.parse_text(quote, self.quote_parse_mode).await?;
                (Some(text), (!entities.is_empty()).then_some(entities))
            }
            None => (None, None),
        };
        let reply = types::InputReplyToMessage {
            reply_to_msg_id: self.message_id,
            top_msg_id: None,
            reply_to_peer_id,
            quote_text,
            quote_entities,
            quote_offset: self.quote_position,
            monoforum_peer_id: None,
            todo_item_id: self.checklist_task_id,
            poll_option: None,
            quote_rich: None,
        };
        Ok(base::InputReplyTo::InputReplyToMessage(Box::new(reply)))
    }
}

impl From<i32> for ReplyParameters {
    fn from(message_id: i32) -> Self {
        ReplyParameters::new(message_id)
    }
}

impl From<&Message> for ReplyParameters {
    fn from(message: &Message) -> Self {
        ReplyParameters::new(message.id).chat_id(message.chat.id)
    }
}

impl From<&Story> for ReplyParameters {
    fn from(story: &Story) -> Self {
        ReplyParameters::story(story.chat.id, story.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raw::Serializable;

    fn seeded() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, base::InputPeer) {
        let (client, rx) = Client::recording();
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, user.clone());
        (client, rx, user)
    }

    #[tokio::test]
    async fn a_story_reply_writes_input_reply_to_story() {
        let (client, mut rx, user) = seeded();
        let written = ReplyParameters::story(7, 8).write(&client, -5).await.unwrap();
        let expected =
            base::InputReplyTo::InputReplyToStory(Box::new(types::InputReplyToStory { peer: user, story_id: 8 }));
        assert_eq!(written.to_bytes(), expected.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_story_reply_with_a_quote_or_task_fails_before_any_request() {
        let (client, mut rx, _) = seeded();
        for reply in [
            ReplyParameters::story(7, 8).quote("hi"),
            ReplyParameters::story(7, 8).quote_position(1),
            ReplyParameters::story(7, 8).checklist_task_id(2),
        ] {
            let err = reply.write(&client, -5).await.unwrap_err();
            assert!(matches!(err, Error::InvalidArgument("a story reply takes no quote or checklist task")), "{err:?}");
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_message_reply_is_unchanged() {
        let (client, _rx, _) = seeded();
        let written = ReplyParameters::new(4).quote_position(2).checklist_task_id(3).write(&client, -5).await.unwrap();
        let expected = types::InputReplyToMessage {
            reply_to_msg_id: 4,
            top_msg_id: None,
            reply_to_peer_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: Some(2),
            monoforum_peer_id: None,
            todo_item_id: Some(3),
            poll_option: None,
            quote_rich: None,
        };
        assert_eq!(written, base::InputReplyTo::InputReplyToMessage(Box::new(expected)));
    }
}
