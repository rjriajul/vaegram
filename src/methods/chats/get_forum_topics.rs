use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::DialogPages;
use crate::raw::{base, functions, types};
use crate::types::{ForumTopic, PeerRef};

pub struct GetForumTopics<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    offset_topic: i32,
    pages: DialogPages<ForumTopic>,
}

impl Client {
    pub fn get_forum_topics(&self, chat_id: impl Into<PeerRef>) -> GetForumTopics<'_> {
        GetForumTopics { client: self, chat_id: chat_id.into(), offset_topic: 0, pages: DialogPages::new() }
    }
}

impl GetForumTopics<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<ForumTopic>> {
        loop {
            if let Some(topic) = self.pages.pop() {
                return Ok(Some(topic));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::GetForumTopics {
                peer,
                q: None,
                offset_date: self.pages.offset_date,
                offset_id: self.pages.offset_id,
                offset_topic: self.offset_topic,
                limit,
            };
            let base::messages::ForumTopics::ForumTopics(page) = self.client.invoke(&request).await?;
            let page = *page;
            let (last_page, next) = (page.topics.len() < limit as usize, cursor(&page));
            Entities::new(self.client, page.users, page.chats);
            let topics = page.topics.into_iter().filter_map(ForumTopic::parse).collect();
            let moved = self.pages.advance(topics, last_page, next.map(|(date, id, _)| (date, id)))?;
            if let (true, Some((_, _, topic))) = (moved, next) {
                self.offset_topic = topic;
            }
        }
    }
}

// A deleted topic has no date or top message to continue from, so the cursor comes from the last one that exists.
fn cursor(page: &types::messages::ForumTopics) -> Option<(i32, i32, i32)> {
    let topic = page.topics.iter().rev().find_map(|t| match t {
        base::ForumTopic::ForumTopic(t) => Some(t),
        base::ForumTopic::ForumTopicDeleted(_) => None,
    })?;
    let message_date = page.messages.iter().find_map(|m| match m {
        base::Message::Message(m) if m.id == topic.top_message => Some(m.date),
        base::Message::MessageService(m) if m.id == topic.top_message => Some(m.date),
        _ => None,
    });
    let date = match message_date {
        Some(date) if !page.order_by_create_date => date,
        _ => topic.date,
    };
    Some((date, topic.top_message, topic.id))
}
