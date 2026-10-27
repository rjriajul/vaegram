use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::advanced::ChatPeer;
use crate::raw::{base, functions};
use crate::types::PeerRef;
use crate::utils::chats_of;

impl Client {
    pub async fn get_chat_members_count(&self, chat_id: impl Into<PeerRef>) -> Result<i32> {
        let count = match self.resolve_chat(&chat_id.into()).await? {
            ChatPeer::Group(chat_id) => {
                let chats = chats_of(self.invoke(&functions::messages::GetChats { id: vec![chat_id] }).await?);
                chats.iter().find_map(|c| match c {
                    base::Chat::Chat(c) if c.id == chat_id => Some(c.participants_count),
                    _ => None,
                })
            }
            channel => match self.full_chat(channel).await?.0 {
                base::ChatFull::ChannelFull(c) => c.participants_count,
                _ => None,
            },
        };
        count.ok_or(Error::Unexpected("Telegram sent no member count"))
    }
}
