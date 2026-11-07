use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

/// Request built by [`Client::unban_chat_member`]. Awaiting it returns `()`.
#[must_use = "a request does nothing until it is awaited"]
pub struct UnbanChatMember<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    user_id: PeerRef,
    only_if_banned: bool,
}

impl Client {
    /// Lifts a ban or restriction in a supergroup or channel; the user has to join again by themselves.
    ///
    /// Unlike the Bot API's `unbanChatMember`, it never removes a current member; use
    /// [`remove_chat_member`](Client::remove_chat_member) for that.
    ///
    /// Fails with `Error::InvalidArgument` for a basic group: it has no ban list, and a removed member can simply
    /// be added back.
    pub fn unban_chat_member(&self, chat_id: impl Into<PeerRef>, user_id: impl Into<PeerRef>) -> UnbanChatMember<'_> {
        UnbanChatMember { client: self, chat_id: chat_id.into(), user_id: user_id.into(), only_if_banned: false }
    }
}

impl UnbanChatMember<'_> {
    /// Does nothing unless the user is banned, so a restricted member keeps their restrictions.
    ///
    /// Costs an extra request (`channels.getParticipant`) to read the member's status first.
    pub fn only_if_banned(mut self) -> Self {
        self.only_if_banned = true;
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let lookup = functions::channels::GetParticipant {
            channel: client.resolve_channel(&self.chat_id).await?,
            participant: client.resolve_peer(self.user_id).await?,
        };
        if self.only_if_banned && !is_banned(client, &lookup).await? {
            return Ok(());
        }
        let functions::channels::GetParticipant { channel, participant } = lookup;
        let request = functions::channels::EditBanned {
            channel,
            participant,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(ChatPermissions::ALL.write(0))),
        };
        client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(UnbanChatMember, ());

/// Whether Telegram lists the participant as kicked: banned from viewing the chat, not only restricted.
/// Someone who never joined is not banned.
async fn is_banned(client: &Client, lookup: &functions::channels::GetParticipant) -> Result<bool> {
    let found = match client.invoke(lookup).await {
        Ok(base::channels::ChannelParticipant::ChannelParticipant(found)) => found.participant,
        Err(Error::Rpc(e)) if e.name == "USER_NOT_PARTICIPANT" => return Ok(false),
        Err(e) => return Err(e),
    };
    Ok(match found {
        base::ChannelParticipant::ChannelParticipantBanned(p) => {
            let base::ChatBannedRights::ChatBannedRights(rights) = &p.banned_rights;
            rights.view_messages
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::ChatPermissions;
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    const USER: i64 = 7;

    fn client(replies: Vec<crate::Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies);
        let mut peers = client.0.peers.lock().unwrap();
        let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
        peers.insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        peers.insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(channel)));
        drop(peers);
        (client, rx)
    }

    fn channel() -> base::InputChannel {
        base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 1 }))
    }

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 1 }))
    }

    /// A `channels.channelParticipant` answer holding a participant banned with `rights`.
    fn banned(rights: types::ChatBannedRights) -> crate::Result<Vec<u8>> {
        let banned = types::ChannelParticipantBanned {
            left: true,
            peer: base::Peer::PeerUser(types::PeerUser { user_id: USER }),
            kicked_by: 1,
            date: 0,
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
            rank: None,
        };
        let found = types::channels::ChannelParticipant {
            participant: base::ChannelParticipant::ChannelParticipantBanned(Box::new(banned)),
            chats: Vec::new(),
            users: Vec::new(),
        };
        Ok(base::channels::ChannelParticipant::ChannelParticipant(Box::new(found)).to_bytes())
    }

    fn unban() -> Vec<u8> {
        let banned_rights = base::ChatBannedRights::ChatBannedRights(Box::new(ChatPermissions::ALL.write(0)));
        functions::channels::EditBanned { channel: channel(), participant: user(), banned_rights }.to_bytes()
    }

    #[tokio::test]
    async fn only_if_banned_unbans_a_banned_member() {
        let mut rights = ChatPermissions::default().write(0);
        rights.view_messages = true;
        let (client, mut rx) = client(vec![banned(rights)]);
        let _ = client.unban_chat_member(get_channel_id(9), USER).only_if_banned().await;
        let lookup = functions::channels::GetParticipant { channel: channel(), participant: user() };
        assert_eq!(rx.try_recv().unwrap(), lookup.to_bytes());
        assert_eq!(rx.try_recv().unwrap(), unban());
    }

    #[tokio::test]
    async fn only_if_banned_leaves_restricted_members_and_non_members_alone() {
        let refused = crate::errors::RpcError::parse(400, "USER_NOT_PARTICIPANT");
        let restricted = ChatPermissions::default().write(0);
        let (client, mut rx) = client(vec![banned(restricted), Err(Error::Rpc(refused))]);
        for _ in 0..2 {
            client.unban_chat_member(get_channel_id(9), USER).only_if_banned().await.unwrap();
            let id = u32::from_le_bytes(rx.try_recv().unwrap()[..4].try_into().unwrap());
            assert_eq!(id, functions::channels::GetParticipant::CONSTRUCTOR_ID);
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn unbans_without_a_lookup_by_default() {
        let (client, mut rx) = client(Vec::new());
        let _ = client.unban_chat_member(get_channel_id(9), USER).await;
        assert_eq!(rx.try_recv().unwrap(), unban());
    }
}
