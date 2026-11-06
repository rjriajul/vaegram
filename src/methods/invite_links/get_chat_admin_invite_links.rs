use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{ChatInviteLink, PeerRef};

const LINKS_LIMIT: usize = 100;

/// Invite links an admin created, built by [`Client::get_chat_admin_invite_links`].
///
/// Awaiting it collects the links into a `Vec`, up to [`limit`](GetChatAdminInviteLinks::limit);
/// [`next`](GetChatAdminInviteLinks::next) reads them one at a time. Links are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatAdminInviteLinks<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    admin_id: PeerRef,
    revoked: bool,
    // Built on the first page and moved along by each one after.
    request: Option<functions::messages::GetExportedChatInvites>,
    seen: usize,
    // The cursor lives in `request`, so the pages only track whether there are more.
    pages: OffsetPages<ChatInviteLink, ()>,
}

impl Client {
    /// Lists the invite links an admin created in a chat.
    pub fn get_chat_admin_invite_links(
        &self,
        chat_id: impl Into<PeerRef>,
        admin_id: impl Into<PeerRef>,
    ) -> GetChatAdminInviteLinks<'_> {
        GetChatAdminInviteLinks {
            client: self,
            chat_id: chat_id.into(),
            admin_id: admin_id.into(),
            revoked: false,
            request: None,
            seen: 0,
            pages: OffsetPages::starting_at(LINKS_LIMIT, ()),
        }
    }
}

impl GetChatAdminInviteLinks<'_> {
    /// Lists the revoked links instead of the active ones.
    pub fn revoked(mut self) -> Self {
        self.revoked = true;
        self
    }

    /// Stops after this many links in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next link, or `None` once the links or the limit run out.
    pub async fn next(&mut self) -> Result<Option<ChatInviteLink>> {
        let client = self.client;
        loop {
            if let Some(link) = self.pages.pop() {
                return Ok(Some(link));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = match self.request.take() {
                Some(request) => request,
                None => functions::messages::GetExportedChatInvites {
                    revoked: self.revoked,
                    peer: client.resolve(&self.chat_id).await?.1,
                    admin_id: client.resolve_user(&self.admin_id).await?,
                    offset_date: None,
                    offset_link: None,
                    limit: 0,
                },
            };
            let request = self.request.insert(request);
            request.limit = limit;
            let base::messages::ExportedChatInvites::ExportedChatInvites(page) = client.invoke(&*request).await?;
            let page = *page;
            self.seen += page.invites.len();
            let next = page.invites.iter().rev().find_map(|invite| match invite {
                base::ExportedChatInvite::ChatInviteExported(i) => Some((i.date, i.link.clone())),
                base::ExportedChatInvite::ChatInvitePublicJoinRequests => None,
            });
            let entities = Entities::new(client, page.users, Vec::new());
            let mut links = Vec::with_capacity(page.invites.len());
            for invite in page.invites {
                match invite {
                    // Not an invite link, so there's nothing to list; it still counts toward `count`.
                    base::ExportedChatInvite::ChatInvitePublicJoinRequests => {}
                    invite => links.push(ChatInviteLink::parse(invite, &entities)?),
                }
            }
            let more = next.and_then(|(date, link)| {
                // Same cursor again would fetch the same page forever.
                let repeated =
                    request.offset_date == Some(date) && request.offset_link.as_deref() == Some(link.as_str());
                let more = self.seen < usize::try_from(page.count).unwrap_or(0) && !repeated;
                more.then(|| (request.offset_date, request.offset_link) = (Some(date), Some(link)))
            });
            self.pages.push(links, more);
        }
    }
}

pager!(GetChatAdminInviteLinks, ChatInviteLink);
