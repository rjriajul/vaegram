use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::advanced::ChatPeer;
use crate::raw::functions;
use crate::types::{ChatPrivileges, PeerRef};

/// Request built by [`Client::promote_chat_member`]. Awaiting it returns `()`.
#[must_use = "a request does nothing until it is awaited"]
pub struct PromoteChatMember<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    user_id: PeerRef,
    privileges: ChatPrivileges,
    title: Option<String>,
}

impl Client {
    /// Sets the administrator privileges of a member.
    ///
    /// In basic groups any privilege makes the user a full admin, and `ChatPrivileges::default()` demotes them.
    pub fn promote_chat_member(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
        privileges: ChatPrivileges,
    ) -> PromoteChatMember<'_> {
        PromoteChatMember { client: self, chat_id: chat_id.into(), user_id: user_id.into(), privileges, title: None }
    }
}

impl PromoteChatMember<'_> {
    /// Gives the admin this custom title, shown instead of "admin" (0-16 characters).
    ///
    /// Only supergroups and channels have titles: with a title set, a basic group fails with
    /// `Error::InvalidArgument` and nothing is sent.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        match client.resolve_chat(&self.chat_id).await? {
            ChatPeer::Group(_) if self.title.is_some() => {
                return Err(Error::InvalidArgument("basic groups have no admin titles; upgrade to a supergroup"));
            }
            ChatPeer::Group(chat_id) => {
                let request = functions::messages::EditChatAdmin {
                    chat_id,
                    user_id: client.resolve_user(&self.user_id).await?,
                    is_admin: self.privileges != ChatPrivileges::default(),
                };
                client.invoke(&request).await?;
            }
            ChatPeer::Channel(channel) => {
                let request = functions::channels::EditAdmin {
                    channel,
                    user_id: client.resolve_user(&self.user_id).await?,
                    admin_rights: self.privileges.write(),
                    rank: self.title,
                };
                client.invoke(&request).await?;
            }
        }
        Ok(())
    }
}

into_future!(PromoteChatMember, ());

#[cfg(test)]
mod tests {
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ChatPrivileges;
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    const USER: i64 = 7;

    fn recording() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        let mut peers = client.0.peers.lock().unwrap();
        let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
        peers.insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        peers.insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(channel)));
        drop(peers);
        (client, rx)
    }

    #[tokio::test]
    async fn title_is_sent_as_the_rank_in_a_channel() {
        let (client, mut rx) = recording();
        let privileges = ChatPrivileges::new().can_pin_messages(true);
        let _ = client.promote_chat_member(get_channel_id(9), USER, privileges).title("Boss").await;
        let request = functions::channels::EditAdmin {
            channel: base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 1 })),
            user_id: base::InputUser::InputUser(Box::new(types::InputUser { user_id: USER, access_hash: 1 })),
            admin_rights: privileges.write(),
            rank: Some("Boss".into()),
        };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn basic_group_title_is_refused_without_a_request() {
        let (client, mut rx) = recording();
        let privileges = ChatPrivileges::new().can_pin_messages(true);
        let result = client.promote_chat_member(-5, USER, privileges).title("Boss").await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }
}
