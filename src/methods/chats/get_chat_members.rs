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

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ChatMembersFilter;
    use crate::raw::functions::{channels, messages};
    use crate::raw::{Serializable, base, types};
    use crate::types::ChatMember;
    use crate::utils::get_channel_id;

    fn ids(members: &[ChatMember]) -> Vec<i64> {
        members.iter().map(|m| m.user.id).collect()
    }

    fn known_channel(client: &Client) -> i64 {
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(peer)));
        get_channel_id(9)
    }

    fn participants(count: i32, user_ids: &[i64]) -> crate::Result<Vec<u8>> {
        let member = |user_id| {
            let member = types::ChannelParticipant { user_id, date: 1, subscription_until_date: None, rank: None };
            base::ChannelParticipant::ChannelParticipant(Box::new(member))
        };
        let page = types::channels::ChannelParticipants {
            count,
            participants: user_ids.iter().map(|&id| member(id)).collect(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        Ok(base::channels::ChannelParticipants::ChannelParticipants(Box::new(page)).to_bytes())
    }

    fn request(offset: i32, limit: i32) -> Vec<u8> {
        let channel = base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 1 }));
        let filter = ChatMembersFilter::Search.write(String::new());
        channels::GetParticipants { channel, filter, offset, limit, hash: 0 }.to_bytes()
    }

    #[tokio::test]
    async fn a_channel_is_read_page_by_page_from_the_offset() {
        let (client, mut sent) = Client::scripted(vec![participants(4, &[1, 2]), participants(4, &[3])]);
        let channel = known_channel(&client);
        let members = client.get_chat_members(channel).offset(1).await.unwrap();
        assert_eq!(ids(&members), [1, 2, 3]);
        assert_eq!(sent.try_recv().unwrap(), request(1, 200));
        assert_eq!(sent.try_recv().unwrap(), request(3, 200));
        // 1 skipped and 3 read reach the 4 Telegram counted.
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_limit_stops_reading_and_not_modified_ends_the_list() {
        let not_modified = Ok(base::channels::ChannelParticipants::ChannelParticipantsNotModified.to_bytes());
        let (client, mut sent) = Client::scripted(vec![participants(9, &[1, 2]), not_modified]);
        let channel = known_channel(&client);
        let mut members = client.get_chat_members(channel).limit(2);
        assert_eq!(members.next().await.unwrap().map(|m| m.user.id), Some(1));
        assert_eq!(members.next().await.unwrap().map(|m| m.user.id), Some(2));
        assert!(members.next().await.unwrap().is_none());
        assert_eq!(sent.try_recv().unwrap(), request(0, 2));

        assert!(client.get_chat_members(channel).await.unwrap().is_empty());
        assert_eq!(sent.try_recv().unwrap(), request(0, 200));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_basic_group_comes_whole_and_skips_in_memory() {
        let member = |user_id| {
            let member = types::ChatParticipant { user_id, inviter_id: 1, date: 1, rank: None };
            base::ChatParticipant::ChatParticipant(Box::new(member))
        };
        let list = types::ChatParticipants { chat_id: 5, participants: (1..=3).map(member).collect(), version: 1 };
        let notify_settings = types::PeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: None,
            ios_sound: None,
            android_sound: None,
            other_sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_ios_sound: None,
            stories_android_sound: None,
            stories_other_sound: None,
        };
        let full = types::ChatFull {
            can_set_username: false,
            has_scheduled: false,
            translations_disabled: false,
            has_welcome_messages: false,
            id: 5,
            about: String::new(),
            participants: base::ChatParticipants::ChatParticipants(Box::new(list)),
            chat_photo: None,
            notify_settings: base::PeerNotifySettings::PeerNotifySettings(Box::new(notify_settings)),
            exported_invite: None,
            bot_info: None,
            pinned_msg_id: None,
            folder_id: None,
            call: None,
            ttl_period: None,
            groupcall_default_join_as: None,
            theme_emoticon: None,
            requests_pending: None,
            recent_requesters: None,
            available_reactions: None,
            reactions_limit: None,
        };
        let reply = types::messages::ChatFull {
            full_chat: base::ChatFull::ChatFull(Box::new(full)),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let (client, mut sent) =
            Client::scripted(vec![Ok(base::messages::ChatFull::ChatFull(Box::new(reply)).to_bytes())]);
        let members = client.get_chat_members(-5_i64).offset(1).limit(1).await.unwrap();
        assert_eq!(ids(&members), [2]);
        assert_eq!(sent.try_recv().unwrap(), messages::GetFullChat { chat_id: 5 }.to_bytes());
        assert!(sent.try_recv().is_err());
    }
}
