use super::pages::Pages;
use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Message;

#[must_use = "call .next() to fetch messages"]
pub struct SearchGlobal<'a> {
    client: &'a Client,
    query: String,
    filter: MessagesFilter,
    broadcasts_only: bool,
    groups_only: bool,
    users_only: bool,
    min_date: i32,
    max_date: i32,
    pages: Pages,
}

impl Client {
    pub fn search_global(&self, query: impl Into<String>) -> SearchGlobal<'_> {
        SearchGlobal {
            client: self,
            query: query.into(),
            filter: MessagesFilter::Empty,
            broadcasts_only: false,
            groups_only: false,
            users_only: false,
            min_date: 0,
            max_date: 0,
            pages: Pages::by_rate(),
        }
    }
}

impl SearchGlobal<'_> {
    pub fn filter(mut self, filter: MessagesFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub fn broadcasts_only(mut self) -> Self {
        self.broadcasts_only = true;
        self
    }

    pub fn groups_only(mut self) -> Self {
        self.groups_only = true;
        self
    }

    pub fn users_only(mut self) -> Self {
        self.users_only = true;
        self
    }

    pub fn min_date(mut self, date: i32) -> Self {
        self.min_date = date;
        self
    }

    pub fn max_date(mut self, date: i32) -> Self {
        self.max_date = date;
        self
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = functions::messages::SearchGlobal {
                broadcasts_only: self.broadcasts_only,
                groups_only: self.groups_only,
                users_only: self.users_only,
                folder_id: None,
                community: None,
                q: self.query.clone(),
                filter: self.filter.write(),
                min_date: self.min_date,
                max_date: self.max_date,
                offset_rate: self.pages.offset_rate,
                offset_peer: self.pages.offset_peer(self.client).await?,
                offset_id: self.pages.offset_id,
                limit,
            };
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
        }
    }
}
