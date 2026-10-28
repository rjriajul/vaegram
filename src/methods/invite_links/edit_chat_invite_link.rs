use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{ChatInviteLink, PeerRef};

/// Options left unset keep their current value.
#[must_use = "a request does nothing until it is awaited"]
pub struct EditChatInviteLink<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    invite_link: String,
    name: Option<String>,
    expire_date: Option<i32>,
    member_limit: Option<i32>,
    creates_join_request: Option<bool>,
}

impl Client {
    pub fn edit_chat_invite_link(
        &self,
        chat_id: impl Into<PeerRef>,
        invite_link: impl Into<String>,
    ) -> EditChatInviteLink<'_> {
        EditChatInviteLink {
            client: self,
            chat_id: chat_id.into(),
            invite_link: invite_link.into(),
            name: None,
            expire_date: None,
            member_limit: None,
            creates_join_request: None,
        }
    }
}

impl EditChatInviteLink<'_> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Unix time after which the link stops working, 0 for never.
    pub fn expire_date(mut self, date: i32) -> Self {
        self.expire_date = Some(date);
        self
    }

    /// 0 removes the limit.
    pub fn member_limit(mut self, limit: i32) -> Self {
        self.member_limit = Some(limit);
        self
    }

    pub fn creates_join_request(mut self, creates_join_request: bool) -> Self {
        self.creates_join_request = Some(creates_join_request);
        self
    }

    async fn send(self) -> Result<ChatInviteLink> {
        let request = functions::messages::EditExportedChatInvite {
            revoked: false,
            peer: self.client.resolve_peer(self.chat_id).await?,
            link: self.invite_link,
            expire_date: self.expire_date,
            usage_limit: self.member_limit,
            request_needed: self.creates_join_request,
            title: self.name,
        };
        ChatInviteLink::parse_edited(self.client, self.client.invoke(&request).await?)
    }
}

into_future!(EditChatInviteLink, ChatInviteLink);
