use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatMembersFilter;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{ChatMember, PeerRef};

const PARTICIPANTS_LIMIT: usize = 200;

/// Members of a chat, built by [`Client::get_chat_members`].
///
/// Awaiting it collects the members into a `Vec`, up to [`limit`](GetChatMembers::limit);
/// [`next`](GetChatMembers::next) reads them one at a time. Channels are read 200 at a time; a basic group comes
/// whole in one request.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatMembers<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query: String,
    filter: ChatMembersFilter,
    // Built once the chat turns out to be a channel.
    request: Option<functions::channels::GetParticipants>,
    pages: OffsetPages<ChatMember, i32>,
}

impl Client {
    /// Lists the members of a chat. Basic groups ignore `query` and `filter`.
    ///
    /// ```no_run
    /// # async fn f(client: &vaegram::Client) -> vaegram::Result<()> {
    /// let members = client.get_chat_members(-1001234567890).limit(50).await?;
    /// # Ok(()) }
    /// ```
    pub fn get_chat_members(&self, chat_id: impl Into<PeerRef>) -> GetChatMembers<'_> {
        GetChatMembers {
            client: self,
            chat_id: chat_id.into(),
            query: String::new(),
            filter: ChatMembersFilter::Search,
            request: None,
            pages: OffsetPages::starting_at(PARTICIPANTS_LIMIT, 0),
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
    /// Only returns channel members whose name matches this text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Selects which participants of a channel to list.
    pub fn filter(mut self, filter: ChatMembersFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Skips this many members from the start of the list.
    pub fn offset(mut self, offset: i32) -> Self {
        self.pages.set_offset(offset);
        self
    }

    /// Stops after this many members in total. The default reads them all.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next member, or `None` once the members or the limit run out.
    pub async fn next(&mut self) -> Result<Option<ChatMember>> {
        let client = self.client;
        loop {
            if let Some(member) = self.pages.pop() {
                return Ok(Some(member));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let offset = self.pages.offset().copied().unwrap_or(0);
            let request = match self.request.take() {
                Some(request) => request,
                None => match client.resolve_chat(&self.chat_id).await? {
                    ChatPeer::Group(chat_id) => {
                        let members = client.basic_group_members(chat_id).await?;
                        let skip = usize::try_from(offset).unwrap_or(0);
                        self.pages.push(members.into_iter().skip(skip).collect(), None);
                        continue;
                    }
                    ChatPeer::Channel(channel) => functions::channels::GetParticipants {
                        channel,
                        filter: self.filter.write(std::mem::take(&mut self.query)),
                        offset: 0,
                        limit: 0,
                        hash: 0,
                    },
                },
            };
            let request = self.request.insert(request);
            (request.offset, request.limit) = (offset, limit);
            let base::channels::ChannelParticipants::ChannelParticipants(page) = client.invoke(&*request).await? else {
                self.pages.push(Vec::new(), None);
                continue;
            };
            let page = *page;
            let entities = Entities::new(client, page.users, page.chats);
            let members = page.participants.iter().map(|p| ChatMember::parse_channel(p, &entities)).collect();
            self.pages.push_counted(members, page.participants.len(), page.count);
        }
    }
}

pager!(GetChatMembers, ChatMember);
