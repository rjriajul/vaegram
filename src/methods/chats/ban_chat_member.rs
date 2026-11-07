use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

/// Request built by [`Client::ban_chat_member`]. Awaiting it returns `()`.
#[must_use = "a request does nothing until it is awaited"]
pub struct BanChatMember<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    user_id: PeerRef,
    until_date: i32,
    revoke_messages: bool,
}

impl Client {
    /// Bans a user or chat from a group, supergroup or channel.
    ///
    /// A basic group has no ban list: it only removes the user, who can be added back by any member allowed to
    /// add members, and ignores `until_date`.
    pub fn ban_chat_member(&self, chat_id: impl Into<PeerRef>, user_id: impl Into<PeerRef>) -> BanChatMember<'_> {
        BanChatMember {
            client: self,
            chat_id: chat_id.into(),
            user_id: user_id.into(),
            until_date: 0,
            revoke_messages: false,
        }
    }
}

impl BanChatMember<'_> {
    /// Ends the ban at this Unix time. 0, or a time less than 30 seconds or more than 366 days away, bans forever.
    pub fn until_date(mut self, until_date: i32) -> Self {
        self.until_date = until_date;
        self
    }

    /// Whether to also delete every message the user sent in the chat. Off by default.
    ///
    /// A basic group does it in the same request. A supergroup or channel needs a second request
    /// (`channels.deleteParticipantHistory`) after the ban: if that one fails, the user stays banned and its error is
    /// returned.
    pub fn revoke_messages(mut self, revoke_messages: bool) -> Self {
        self.revoke_messages = revoke_messages;
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        match client.resolve_chat(&self.chat_id).await? {
            ChatPeer::Group(chat_id) => {
                let user_id = client.resolve_user(&self.user_id).await?;
                let request =
                    functions::messages::DeleteChatUser { revoke_history: self.revoke_messages, chat_id, user_id };
                client.invoke(&request).await?;
            }
            ChatPeer::Channel(channel) => {
                let mut rights = ChatPermissions::default().write(self.until_date);
                rights.view_messages = true;
                let request = functions::channels::EditBanned {
                    channel,
                    participant: client.resolve_peer(self.user_id).await?,
                    banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
                };
                client.invoke(&request).await?;
                if self.revoke_messages {
                    let functions::channels::EditBanned { channel, participant, .. } = request;
                    let chat_id = client.peer_id(self.chat_id).await?;
                    let request = functions::channels::DeleteParticipantHistory { channel, participant };
                    client.affected_history(chat_id, &request).await?;
                    client.0.caches.evict_chat(chat_id);
                }
            }
        }
        Ok(())
    }
}

into_future!(BanChatMember, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ChatPermissions;
    use crate::utils::get_channel_id;

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

    #[tokio::test]
    async fn basic_group_revokes_in_the_same_request() {
        let (client, mut rx) = client(Vec::new());
        let _ = client.ban_chat_member(-5, USER).revoke_messages(true).await;
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: USER, access_hash: 1 }));
        let request = functions::messages::DeleteChatUser { revoke_history: true, chat_id: 5, user_id };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn supergroup_deletes_the_history_after_the_ban() {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let affected = types::messages::AffectedHistory { pts: 3, pts_count: 1, offset: 0 };
        let replies = vec![
            Ok(base::Updates::Updates(Box::new(updates)).to_bytes()),
            Ok(base::messages::AffectedHistory::AffectedHistory(Box::new(affected)).to_bytes()),
        ];
        let (client, mut rx) = client(replies);
        client.ban_chat_member(get_channel_id(9), USER).revoke_messages(true).await.unwrap();

        let mut rights = ChatPermissions::default().write(0);
        rights.view_messages = true;
        let banned_rights = base::ChatBannedRights::ChatBannedRights(Box::new(rights));
        let ban = functions::channels::EditBanned { channel: channel(), participant: user(), banned_rights };
        assert_eq!(rx.try_recv().unwrap(), ban.to_bytes());
        let history = functions::channels::DeleteParticipantHistory { channel: channel(), participant: user() };
        assert_eq!(rx.try_recv().unwrap(), history.to_bytes());
        assert!(rx.try_recv().is_err());
    }
}
