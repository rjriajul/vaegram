use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Chat, PeerRef};
use crate::utils::{get_channel_id, get_peer};

impl Client {
    pub async fn join_chat(&self, chat_id: impl Into<PeerRef>) -> Result<Chat> {
        let (known_id, result) = match chat_id.into() {
            PeerRef::Invite(hash) => {
                (None, self.invoke(&functions::messages::ImportChatInvite { hash: hash.into() }).await?)
            }
            chat => {
                let chat_id = self.peer_id(chat).await?;
                let channel = self.resolve_channel(&chat_id.into()).await?;
                (Some(chat_id), self.invoke(&functions::channels::JoinChannel { channel }).await?)
            }
        };
        let base::messages::ChatInviteJoinResult::ChatInviteJoinResultOk(ok) = result else {
            return Err(Error::Unexpected("this chat asks to join through a bot web app"));
        };
        let (users, chats) = match ok.updates {
            base::Updates::Updates(u) => (u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.users, u.chats),
            _ => return Err(Error::Unexpected("Telegram sent no chat back")),
        };
        let id = known_id
            .or_else(|| {
                chats.iter().find_map(|c| match c {
                    base::Chat::Chat(c) if !c.left => Some(-c.id),
                    base::Chat::Channel(c) if !c.left && !c.min => Some(get_channel_id(c.id)),
                    _ => None,
                })
            })
            .ok_or(Error::Unexpected("Telegram sent no chat back"))?;
        let entities = Entities::new(self, users, chats);
        Ok(Chat::parse(&get_peer(id), &entities, false))
    }
}
