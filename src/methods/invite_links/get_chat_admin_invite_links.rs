use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatInviteLink, PeerRef};

const LINKS_LIMIT: usize = 100;

#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatAdminInviteLinks<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    admin_id: PeerRef,
    revoked: bool,
    limit: usize,
}

impl Client {
    /// Fetches every page before returning; `limit` caps it.
    pub fn get_chat_admin_invite_links(
        &self,
        chat_id: impl Into<PeerRef>,
        admin_id: impl Into<PeerRef>,
    ) -> GetChatAdminInviteLinks<'_> {
        let (chat_id, admin_id) = (chat_id.into(), admin_id.into());
        GetChatAdminInviteLinks { client: self, chat_id, admin_id, revoked: false, limit: usize::MAX }
    }
}

impl GetChatAdminInviteLinks<'_> {
    /// Lists the revoked links instead of the active ones.
    pub fn revoked(mut self) -> Self {
        self.revoked = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<Vec<ChatInviteLink>> {
        let client = self.client;
        let mut request = functions::messages::GetExportedChatInvites {
            revoked: self.revoked,
            peer: client.resolve_peer(self.chat_id).await?,
            admin_id: client.resolve_user(&self.admin_id).await?,
            offset_date: None,
            offset_link: None,
            limit: 0,
        };
        let (mut links, mut seen) = (Vec::new(), 0);
        while links.len() < self.limit {
            request.limit = LINKS_LIMIT.min(self.limit - links.len()) as i32;
            let base::messages::ExportedChatInvites::ExportedChatInvites(page) = client.invoke(&request).await?;
            let page = *page;
            seen += page.invites.len();
            let next = page.invites.iter().rev().find_map(|invite| match invite {
                base::ExportedChatInvite::ChatInviteExported(i) => Some((i.date, i.link.clone())),
                base::ExportedChatInvite::ChatInvitePublicJoinRequests => None,
            });
            let entities = Entities::new(client, page.users, Vec::new());
            for invite in page.invites {
                match invite {
                    // Not an invite link, so there's nothing to list; it still counts toward `count`.
                    base::ExportedChatInvite::ChatInvitePublicJoinRequests => {}
                    invite => links.push(ChatInviteLink::parse(invite, &entities)?),
                }
            }
            let Some((date, link)) = next else { break };
            // Same cursor again would fetch the same page forever.
            let repeated = request.offset_date == Some(date) && request.offset_link.as_deref() == Some(link.as_str());
            if seen >= page.count as usize || repeated {
                break;
            }
            (request.offset_date, request.offset_link) = (Some(date), Some(link));
        }
        Ok(links)
    }
}

into_future!(GetChatAdminInviteLinks, Vec<ChatInviteLink>);
