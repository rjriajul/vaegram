use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{GroupCallMember, PeerRef};

#[must_use = "call .next() to fetch call members"]
pub struct GetCallMembers<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    call: Option<base::InputGroupCall>,
    pages: OffsetPages<GroupCallMember>,
}

impl Client {
    /// Lists the members of the group's or channel's active video chat; without one the list is empty.
    pub fn get_call_members(&self, chat_id: impl Into<PeerRef>) -> GetCallMembers<'_> {
        GetCallMembers { client: self, chat_id: chat_id.into(), call: None, pages: OffsetPages::new(100) }
    }
}

impl GetCallMembers<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<GroupCallMember>> {
        loop {
            if let Some(member) = self.pages.pop() {
                return Ok(Some(member));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let call = match &self.call {
                Some(call) => call.clone(),
                None => {
                    let chat = self.client.resolve_chat(&self.chat_id).await?;
                    let call = match self.client.full_chat(chat).await?.0 {
                        base::ChatFull::ChatFull(c) => c.call,
                        base::ChatFull::ChannelFull(c) => c.call,
                        base::ChatFull::CommunityFull(_) => None,
                    };
                    let Some(call) = call else {
                        self.pages.remaining = 0;
                        return Ok(None);
                    };
                    self.call = Some(call.clone());
                    call
                }
            };
            let offset = self.pages.next_offset().unwrap_or_default().into();
            let request =
                functions::phone::GetGroupParticipants { call, ids: Vec::new(), sources: Vec::new(), offset, limit };
            let base::phone::GroupParticipants::GroupParticipants(page) = self.client.invoke(&request).await?;
            let page = *page;
            let entities = Entities::new(self.client, page.users, page.chats);
            let parse = |raw: Vec<base::GroupCallParticipant>| {
                raw.into_iter().map(|m| GroupCallMember::parse(m, &entities)).collect()
            };
            self.pages.refill(page.participants, Some(page.next_offset), parse);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::{channels, messages, phone};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::utils::get_channel_id;

    fn call() -> base::InputGroupCall {
        base::InputGroupCall::InputGroupCall(Box::new(types::InputGroupCall { id: 3, access_hash: 4 }))
    }

    fn member(user_id: i64) -> base::GroupCallParticipant {
        let member = types::GroupCallParticipant {
            muted: false,
            left: false,
            can_self_unmute: false,
            just_joined: false,
            versioned: false,
            min: false,
            muted_by_you: false,
            volume_by_admin: false,
            is_self: false,
            video_joined: false,
            peer: base::Peer::PeerUser(types::PeerUser { user_id }),
            date: 10,
            active_date: None,
            source: 1,
            volume: None,
            about: None,
            raise_hand_rating: None,
            video: None,
            presentation: None,
            paid_stars_total: None,
        };
        base::GroupCallParticipant::GroupCallParticipant(Box::new(member))
    }

    fn named_user(id: i64) -> base::User {
        let mut buf = Vec::new();
        // Only access_hash and first_name are set.
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        "member".to_owned().serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn empty_vector() -> Vec<u8> {
        Vec::<i64>::new().to_bytes()
    }

    fn notify_settings(buf: &mut Vec<u8>) {
        types::PeerNotifySettings::CONSTRUCTOR_ID.serialize(buf);
        0u32.serialize(buf);
    }

    // messages.chatFull wrapping a channelFull with only its required fields and `call`.
    fn channel_full() -> Vec<u8> {
        let mut buf = Vec::new();
        types::messages::ChatFull::CONSTRUCTOR_ID.serialize(&mut buf);
        types::ChannelFull::CONSTRUCTOR_ID.serialize(&mut buf);
        (1u32 << 21).serialize(&mut buf);
        0u32.serialize(&mut buf);
        9i64.serialize(&mut buf);
        String::new().serialize(&mut buf);
        for n in [0i32, 0, 0] {
            n.serialize(&mut buf);
        }
        base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 }).serialize(&mut buf);
        notify_settings(&mut buf);
        buf.extend(empty_vector());
        0i32.serialize(&mut buf);
        call().serialize(&mut buf);
        buf.extend(empty_vector());
        buf.extend(empty_vector());
        buf
    }

    // messages.chatFull wrapping a basic group's chatFull, with or without `call`.
    fn chat_full(with_call: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        types::messages::ChatFull::CONSTRUCTOR_ID.serialize(&mut buf);
        types::ChatFull::CONSTRUCTOR_ID.serialize(&mut buf);
        (u32::from(with_call) << 12).serialize(&mut buf);
        5i64.serialize(&mut buf);
        String::new().serialize(&mut buf);
        let participants = types::ChatParticipantsForbidden { chat_id: 5, self_participant: None };
        base::ChatParticipants::ChatParticipantsForbidden(Box::new(participants)).serialize(&mut buf);
        notify_settings(&mut buf);
        if with_call {
            call().serialize(&mut buf);
        }
        buf.extend(empty_vector());
        buf.extend(empty_vector());
        buf
    }

    fn page(user_ids: &[i64], next_offset: &str) -> Vec<u8> {
        let page = types::phone::GroupParticipants {
            count: 3,
            participants: user_ids.iter().map(|&id| member(id)).collect(),
            next_offset: next_offset.into(),
            chats: Vec::new(),
            users: user_ids.iter().map(|&id| named_user(id)).collect(),
            version: 1,
        };
        base::phone::GroupParticipants::GroupParticipants(Box::new(page)).to_bytes()
    }

    fn participants(offset: &str) -> Vec<u8> {
        let request = phone::GetGroupParticipants {
            call: call(),
            ids: Vec::new(),
            sources: Vec::new(),
            offset: offset.into(),
            limit: 100,
        };
        request.to_bytes()
    }

    fn known_channel(client: &Client) -> i64 {
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(peer)));
        get_channel_id(9)
    }

    #[tokio::test]
    async fn walks_pages_until_the_offset_runs_out() {
        let replies = vec![Ok(channel_full()), Ok(page(&[1, 2], "n")), Ok(page(&[3], ""))];
        let (client, mut rx) = Client::scripted(replies);
        let channel = known_channel(&client);
        let mut members = client.get_call_members(channel);
        let mut ids = Vec::new();
        while let Some(member) = members.next().await.unwrap() {
            ids.push((member.chat.id, member.chat.first_name));
        }
        let member = |id| (id, Some("member".to_owned()));
        assert_eq!(ids, [member(1), member(2), member(3)]);

        let input = types::InputChannel { channel_id: 9, access_hash: 1 };
        let full = channels::GetFullChannel { channel: base::InputChannel::InputChannel(Box::new(input)) };
        assert_eq!(rx.try_recv().unwrap(), full.to_bytes());
        assert_eq!(rx.try_recv().unwrap(), participants(""));
        assert_eq!(rx.try_recv().unwrap(), participants("n"));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_basic_group_uses_get_full_chat() {
        let (client, mut rx) = Client::scripted(vec![Ok(chat_full(true)), Ok(page(&[1], ""))]);
        let member = client.get_call_members(-5_i64).next().await.unwrap();
        assert_eq!(member.map(|m| m.chat.id), Some(1));
        assert_eq!(rx.try_recv().unwrap(), messages::GetFullChat { chat_id: 5 }.to_bytes());
        assert_eq!(rx.try_recv().unwrap(), participants(""));
    }

    #[tokio::test]
    async fn no_active_call_means_no_members() {
        // The group has no video chat, so there's nobody to list.
        let (client, mut rx) = Client::scripted(vec![Ok(chat_full(false))]);
        let mut members = client.get_call_members(-5_i64);
        assert_eq!(members.next().await.unwrap(), None);
        assert_eq!(members.next().await.unwrap(), None);
        assert_eq!(rx.try_recv().unwrap(), messages::GetFullChat { chat_id: 5 }.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_user_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let result = client.get_call_members(7_i64).next().await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn limit_stops_early() {
        let (client, mut rx) = Client::scripted(vec![Ok(channel_full()), Ok(page(&[1, 2], "n"))]);
        let channel = known_channel(&client);
        let mut members = client.get_call_members(channel).limit(1);
        assert_eq!(members.next().await.unwrap().map(|m| m.chat.id), Some(1));
        assert_eq!(members.next().await.unwrap(), None);
        let _ = rx.try_recv().unwrap();
        let limited = phone::GetGroupParticipants {
            call: call(),
            ids: Vec::new(),
            sources: Vec::new(),
            offset: String::new(),
            limit: 1,
        };
        assert_eq!(rx.try_recv().unwrap(), limited.to_bytes());
        assert!(rx.try_recv().is_err());
    }
}
