use crate::Client;
use crate::errors::Result;
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::{ChatPermissions, PeerRef};

/// Request built by [`Client::kick_chat_member`]. Awaiting it returns `()`.
#[must_use = "a request does nothing until it is awaited"]
pub struct KickChatMember<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    user_id: PeerRef,
}

impl Client {
    /// Removes a user or chat from a group, supergroup or channel without banning them, so they can join again.
    ///
    /// A basic group removes the member in one request. A supergroup or channel has no such request, so this bans
    /// and then unbans, which also lifts any restriction the member had. It is not atomic: if the unban fails, the
    /// member stays banned and its error is returned.
    pub fn kick_chat_member(&self, chat_id: impl Into<PeerRef>, user_id: impl Into<PeerRef>) -> KickChatMember<'_> {
        KickChatMember { client: self, chat_id: chat_id.into(), user_id: user_id.into() }
    }
}

impl KickChatMember<'_> {
    async fn send(self) -> Result<()> {
        let client = self.client;
        match client.resolve_chat(&self.chat_id).await? {
            ChatPeer::Group(chat_id) => {
                let user_id = client.resolve_user(&self.user_id).await?;
                client.invoke(&functions::messages::DeleteChatUser { revoke_history: false, chat_id, user_id }).await?;
            }
            ChatPeer::Channel(channel) => {
                let mut rights = ChatPermissions::default().write(0);
                rights.view_messages = true;
                let mut request = functions::channels::EditBanned {
                    channel,
                    participant: client.resolve_peer(self.user_id).await?,
                    banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
                };
                client.invoke(&request).await?;
                let unbanned = ChatPermissions::ALL.write(0);
                request.banned_rights = base::ChatBannedRights::ChatBannedRights(Box::new(unbanned));
                client.invoke(&request).await?;
            }
        }
        Ok(())
    }
}

into_future!(KickChatMember, ());

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

    fn updates() -> crate::Result<Vec<u8>> {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        Ok(base::Updates::Updates(Box::new(updates)).to_bytes())
    }

    #[tokio::test]
    async fn basic_group_deletes_the_member() {
        let (client, mut rx) = client(vec![updates()]);
        client.kick_chat_member(-5, USER).await.unwrap();
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: USER, access_hash: 1 }));
        let request = functions::messages::DeleteChatUser { revoke_history: false, chat_id: 5, user_id };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn supergroup_bans_then_unbans() {
        let (client, mut rx) = client(vec![updates(), updates()]);
        client.kick_chat_member(get_channel_id(9), USER).await.unwrap();
        let edit = |rights| functions::channels::EditBanned {
            channel: base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 1 })),
            participant: base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser {
                user_id: USER,
                access_hash: 1,
            })),
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
        };
        let mut banned = ChatPermissions::default().write(0);
        banned.view_messages = true;
        assert_eq!(rx.try_recv().unwrap(), edit(banned).to_bytes());
        assert_eq!(rx.try_recv().unwrap(), edit(ChatPermissions::ALL.write(0)).to_bytes());
        assert!(rx.try_recv().is_err());
    }
}
