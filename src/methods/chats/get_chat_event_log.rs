use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatEvent, ChatEventFilter, PeerRef};

const PAGE_SIZE: usize = 100;

pub struct GetChatEventLog<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query: String,
    filter: ChatEventFilter,
    user_ids: Vec<PeerRef>,
    max_id: i64,
    remaining: usize,
    buffer: std::vec::IntoIter<base::ChannelAdminLogEvent>,
    entities: Entities,
    done: bool,
}

impl Client {
    pub fn get_chat_event_log(&self, chat_id: impl Into<PeerRef>) -> GetChatEventLog<'_> {
        GetChatEventLog {
            client: self,
            chat_id: chat_id.into(),
            query: String::new(),
            filter: ChatEventFilter::default(),
            user_ids: Vec::new(),
            max_id: 0,
            remaining: usize::MAX,
            buffer: Vec::new().into_iter(),
            entities: Entities::default(),
            done: false,
        }
    }
}

impl GetChatEventLog<'_> {
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn filter(mut self, filter: ChatEventFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn user_ids(mut self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Self {
        self.user_ids = user_ids.into_iter().map(Into::into).collect();
        self
    }

    pub fn offset_id(mut self, event_id: i64) -> Self {
        self.max_id = event_id;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<ChatEvent>> {
        while self.remaining > 0 {
            if let Some(event) = self.buffer.next() {
                self.remaining -= 1;
                return Ok(Some(ChatEvent::parse(self.client, event, &self.entities)));
            }
            if self.done {
                break;
            }
            let base::channels::AdminLogResults::AdminLogResults(page) =
                self.client.invoke(&self.build().await?).await?;
            let page = *page;
            match page.events.last() {
                // An empty page means the log has ended. A last id that isn't below max_id would ask for the same
                // page again, so the walk stops there instead of looping on a server that ignores max_id.
                Some(base::ChannelAdminLogEvent::ChannelAdminLogEvent(e)) if self.max_id == 0 || e.id < self.max_id => {
                    self.max_id = e.id
                }
                _ => self.done = true,
            }
            self.entities = Entities::new(self.client, page.users, page.chats);
            self.buffer = if self.done { Vec::new() } else { page.events }.into_iter();
        }
        Ok(None)
    }

    async fn build(&mut self) -> Result<functions::channels::GetAdminLog> {
        self.chat_id = PeerRef::Id(self.client.resolve(&self.chat_id).await?.0);
        let channel = self.client.resolve_channel(&self.chat_id).await?;
        let mut admins = Vec::with_capacity(self.user_ids.len());
        for user in &self.user_ids {
            admins.push(self.client.resolve_user(user).await?);
        }
        Ok(functions::channels::GetAdminLog {
            channel,
            q: self.query.clone(),
            events_filter: self.filter.write(),
            admins: (!admins.is_empty()).then_some(admins),
            max_id: self.max_id,
            min_id: 0,
            limit: self.remaining.min(PAGE_SIZE) as i32,
        })
    }
}
