use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{ChatJoiner, PeerRef};

const IMPORTERS_LIMIT: usize = 100;

/// Pending join requests of a chat, built by [`Client::get_chat_join_requests`].
///
/// Awaiting it collects the requests into a `Vec`, up to [`limit`](GetChatJoinRequests::limit);
/// [`next`](GetChatJoinRequests::next) reads them one at a time. Requests are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatJoinRequests<'a> {
    client: &'a Client,
    importers: Importers,
}

impl Client {
    /// Lists the requests to join a chat. User accounts only.
    pub fn get_chat_join_requests(&self, chat_id: impl Into<PeerRef>) -> GetChatJoinRequests<'_> {
        GetChatJoinRequests { client: self, importers: Importers::new(chat_id.into(), true, None) }
    }
}

impl GetChatJoinRequests<'_> {
    /// Only returns requests matching this text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.importers.query = Some(query.into());
        self
    }

    /// Stops after this many requests in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.importers.pages.remaining = limit;
        self
    }

    /// Returns the next request, or `None` once the requests or the limit run out.
    pub async fn next(&mut self) -> Result<Option<ChatJoiner>> {
        self.importers.next(self.client).await
    }
}

pager!(GetChatJoinRequests, ChatJoiner);

/// Pages of the users who joined a chat or asked to, each page starting after the last (date, user) pair read.
pub(super) struct Importers {
    chat_id: PeerRef,
    requested: bool,
    link: Option<String>,
    query: Option<String>,
    // Built on the first page and moved along by each one after.
    request: Option<functions::messages::GetChatInviteImporters>,
    read: usize,
    // The (date, user) pair the last request continued from; the server repeating it would loop forever.
    cursor: Option<(i32, i64)>,
    // The cursor lives in `request`, so the pages only track whether there are more.
    pub(super) pages: OffsetPages<ChatJoiner, ()>,
}

impl Importers {
    pub(super) fn new(chat_id: PeerRef, requested: bool, link: Option<String>) -> Self {
        let pages = OffsetPages::starting_at(IMPORTERS_LIMIT, ());
        Importers { chat_id, requested, link, query: None, request: None, read: 0, cursor: None, pages }
    }

    pub(super) async fn next(&mut self, client: &Client) -> Result<Option<ChatJoiner>> {
        loop {
            if let Some(joiner) = self.pages.pop() {
                return Ok(Some(joiner));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = match self.request.take() {
                Some(request) => request,
                None => functions::messages::GetChatInviteImporters {
                    requested: self.requested,
                    subscription_expired: false,
                    peer: client.resolve(&self.chat_id).await?.1,
                    link: self.link.take(),
                    q: self.query.take(),
                    offset_date: 0,
                    offset_user: base::InputUser::InputUserEmpty,
                    limit: 0,
                },
            };
            let request = self.request.insert(request);
            request.limit = limit;
            let base::messages::ChatInviteImporters::ChatInviteImporters(page) = client.invoke(&*request).await?;
            let page = *page;
            let fetched = page.importers.len();
            self.read += fetched;
            let last = page.importers.last().map(|base::ChatInviteImporter::ChatInviteImporter(i)| (i.date, i.user_id));
            let entities = Entities::new(client, page.users, Vec::new());
            let joiners = page
                .importers
                .into_iter()
                .map(|base::ChatInviteImporter::ChatInviteImporter(i)| ChatJoiner::parse(*i, &entities))
                .collect();
            let more = match last {
                // Without room for another page, the cursor would only cost a lookup that may fail on an empty user.
                Some((date, user_id))
                    if self.read < usize::try_from(page.count).unwrap_or(0)
                        && self.pages.remaining > fetched
                        && self.cursor != Some((date, user_id)) =>
                {
                    // The next page starts after this (date, user) pair; the user's access hash was just cached.
                    self.cursor = Some((date, user_id));
                    request.offset_date = date;
                    request.offset_user = client.resolve_user(&user_id.into()).await?;
                    Some(())
                }
                // An empty page ends the list too: asking again would loop forever.
                _ => None,
            };
            self.pages.push(joiners, more);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::create_chat_invite_link::tests::{CHANNEL_ID, channel, setup};
    use crate::raw::functions::messages;
    use crate::raw::{Deserializable, Serializable, base, types};
    use crate::utils::get_channel_id;

    fn user(id: i64) -> base::User {
        let mut bytes = Vec::new();
        // user#b1b8cc83, flags with only access_hash set, empty flags2; every other field is optional.
        for word in [0xb1b8cc83, 1, 0] {
            bytes.extend(u32::to_le_bytes(word));
        }
        bytes.extend(id.to_le_bytes());
        bytes.extend((id * 10).to_le_bytes());
        base::User::from_bytes(&bytes).unwrap()
    }

    pub(crate) fn page(count: i32, joiners: &[(i64, i32)]) -> Vec<u8> {
        let (mut importers, mut users) = (Vec::new(), Vec::new());
        for &(user_id, date) in joiners {
            let importer = types::ChatInviteImporter {
                requested: true,
                via_chatlist: false,
                user_id,
                date,
                about: None,
                approved_by: None,
            };
            importers.push(base::ChatInviteImporter::ChatInviteImporter(Box::new(importer)));
            users.push(user(user_id));
        }
        let page = types::messages::ChatInviteImporters { count, importers, users };
        base::messages::ChatInviteImporters::ChatInviteImporters(Box::new(page)).to_bytes()
    }

    #[tokio::test]
    async fn pages_join_requests_after_last_joiner() {
        let (client, mut rx) = setup(vec![page(3, &[(10, 300), (11, 200)]), page(3, &[(12, 100)])]);
        let joiners = client.get_chat_join_requests(get_channel_id(CHANNEL_ID)).query("a").await.unwrap();
        let joiners: Vec<_> = joiners.iter().map(|j| (j.user.id, j.date)).collect();
        assert_eq!(joiners, [(10, 300), (11, 200), (12, 100)]);

        let mut request = messages::GetChatInviteImporters {
            requested: true,
            subscription_expired: false,
            peer: channel(),
            link: None,
            q: Some("a".into()),
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit: 100,
        };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        request.offset_date = 200;
        request.offset_user = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 11, access_hash: 110 }));
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_limit_stops_reading_and_an_empty_page_ends_the_list() {
        let (client, mut rx) = setup(vec![page(3, &[(10, 300), (11, 200)]), page(5, &[])]);
        let mut requests = client.get_chat_join_requests(get_channel_id(CHANNEL_ID)).limit(1);
        assert_eq!(requests.next().await.unwrap().map(|j| j.user.id), Some(10));
        assert!(requests.next().await.unwrap().is_none());
        assert!(client.get_chat_join_requests(get_channel_id(CHANNEL_ID)).await.unwrap().is_empty());
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
        assert_eq!(sent.len(), 2);
        let request = |limit| messages::GetChatInviteImporters {
            requested: true,
            subscription_expired: false,
            peer: channel(),
            link: None,
            q: None,
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit,
        };
        assert_eq!(sent, [request(1).to_bytes(), request(100).to_bytes()]);
    }

    #[tokio::test]
    async fn a_repeated_cursor_ends_the_list() {
        let same = || page(i32::MAX, &[(10, 200)]);
        let (client, mut rx) = setup(vec![same(), same(), same()]);
        client.get_chat_join_requests(get_channel_id(CHANNEL_ID)).await.unwrap();
        assert_eq!(std::iter::from_fn(|| rx.try_recv().ok()).count(), 2);
    }

    #[tokio::test]
    async fn the_last_item_of_the_limit_needs_no_cursor() {
        // The user is missing from the page, so looking up the cursor would fail.
        let empty = types::UserEmpty { id: 10 };
        let importer = types::ChatInviteImporter {
            requested: true,
            via_chatlist: false,
            user_id: 10,
            date: 200,
            about: None,
            approved_by: None,
        };
        let reply = types::messages::ChatInviteImporters {
            count: 2,
            importers: vec![base::ChatInviteImporter::ChatInviteImporter(Box::new(importer))],
            users: vec![base::User::UserEmpty(empty)],
        };
        let reply = base::messages::ChatInviteImporters::ChatInviteImporters(Box::new(reply)).to_bytes();
        let (client, _rx) = setup(vec![reply]);
        let found = client.get_chat_join_requests(get_channel_id(CHANNEL_ID)).limit(1).await.unwrap();
        assert_eq!(found.iter().map(|j| j.user.id).collect::<Vec<_>>(), [10]);
    }
}
