use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatMembersFilter;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::ChatMember;

const PARTICIPANTS_LIMIT: usize = 200;

pub struct GetChatMembers<'a> {
    client: &'a Client,
    chat_id: i64,
    query: String,
    filter: ChatMembersFilter,
    limit: usize,
}

impl Client {
    /// Basic groups ignore `query` and `filter`.
    pub fn get_chat_members(&self, chat_id: i64) -> GetChatMembers<'_> {
        GetChatMembers {
            client: self,
            chat_id,
            query: String::new(),
            filter: ChatMembersFilter::Search,
            limit: usize::MAX,
        }
    }

    pub(super) async fn basic_group_members(&self, chat_id: i64) -> Result<Vec<ChatMember>> {
        let (full_chat, entities) = self.full_chat(ChatPeer::Group(chat_id)).await?;
        let participants: &[base::ChatParticipant] = match &full_chat {
            base::ChatFull::ChatFull(c) => match &c.participants {
                base::ChatParticipants::ChatParticipants(p) => p.participants.as_slice(),
                base::ChatParticipants::ChatParticipantsForbidden(p) => p.self_participant.as_slice(),
            },
            _ => &[],
        };
        Ok(participants.iter().map(|p| ChatMember::parse_chat(p, &entities)).collect())
    }
}

impl GetChatMembers<'_> {
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn filter(mut self, filter: ChatMembersFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<Vec<ChatMember>> {
        let client = self.client;
        let channel = match client.resolve_chat(self.chat_id).await? {
            ChatPeer::Group(chat_id) => {
                let mut members = client.basic_group_members(chat_id).await?;
                members.truncate(self.limit);
                return Ok(members);
            }
            ChatPeer::Channel(channel) => channel,
        };
        let mut request = functions::channels::GetParticipants {
            channel,
            filter: self.filter.write(self.query),
            offset: 0,
            limit: 0,
            hash: 0,
        };
        let mut members = Vec::new();
        while members.len() < self.limit {
            request.offset = members.len() as i32;
            request.limit = PARTICIPANTS_LIMIT.min(self.limit - members.len()) as i32;
            let base::channels::ChannelParticipants::ChannelParticipants(page) = client.invoke(&request).await? else {
                break;
            };
            let page = *page;
            if page.participants.is_empty() {
                break;
            }
            let entities = Entities::new(client, page.users, page.chats);
            members.extend(page.participants.iter().map(|p| ChatMember::parse_channel(p, &entities)));
            if members.len() >= page.count as usize {
                break;
            }
        }
        Ok(members)
    }
}

into_future!(GetChatMembers, Vec<ChatMember>);
