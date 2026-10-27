use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::Chat;
use crate::utils::{get_channel_id, get_peer};

impl Client {
    /// Joins by `@username`, `t.me/username` or an invite link (`t.me/+…`, `t.me/joinchat/…`).
    pub async fn join_chat(&self, chat: &str) -> Result<Chat> {
        let (known_id, result) = match invite_hash(chat) {
            Some(hash) => (None, self.invoke(&functions::messages::ImportChatInvite { hash: hash.into() }).await?),
            None => {
                let username = chat.rsplit_once('/').map_or(chat, |(_, username)| username);
                let chat_id = self.resolve_username(username).await?;
                let channel = self.resolve_channel(chat_id).await?;
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

fn invite_hash(link: &str) -> Option<&str> {
    let (_, hash) = link.split_once("joinchat/").or_else(|| link.split_once('+'))?;
    Some(hash)
}

#[cfg(test)]
mod tests {
    use super::invite_hash;

    #[test]
    fn finds_invite_hash() {
        assert_eq!(invite_hash("https://t.me/+AbC-d_1"), Some("AbC-d_1"));
        assert_eq!(invite_hash("t.me/joinchat/AbC"), Some("AbC"));
        assert_eq!(invite_hash("+AbC"), Some("AbC"));
        assert_eq!(invite_hash("@durov"), None);
        assert_eq!(invite_hash("https://t.me/durov"), None);
    }
}
