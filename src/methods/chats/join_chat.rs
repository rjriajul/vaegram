use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Chat, PeerRef, User};
use crate::utils::{get_channel_id, get_peer};

impl Client {
    /// Joins a public channel or supergroup, or a chat through an invite link.
    ///
    /// An invite link works as `chat_id` too, as `PeerRef::Invite`. Returns the joined `Chat`. Fails with
    /// [`Error::GuardBotApprovalRequired`] when a guard bot has to approve the join first.
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
        joined_chat(self, known_id, result)
    }
}

fn joined_chat(client: &Client, known_id: Option<i64>, result: base::messages::ChatInviteJoinResult) -> Result<Chat> {
    let ok = match result {
        base::messages::ChatInviteJoinResult::ChatInviteJoinResultOk(ok) => ok,
        base::messages::ChatInviteJoinResult::ChatInviteJoinResultWebView(web) => {
            let entities = Entities::new(client, web.users, Vec::new());
            return Err(Error::GuardBotApprovalRequired {
                bot: Box::new(User::parse(web.bot_id, &entities)),
                query_id: web.query_id,
            });
        }
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
    let entities = Entities::new(client, users, chats);
    Ok(Chat::parse(&get_peer(id), &entities, false))
}

#[cfg(test)]
mod tests {
    use super::joined_chat;
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};

    #[test]
    fn web_view_join_names_the_guard_bot() {
        let web = types::messages::ChatInviteJoinResultWebView { bot_id: 77, query_id: 5, users: Vec::new() };
        let result = base::messages::ChatInviteJoinResult::ChatInviteJoinResultWebView(Box::new(web));
        match joined_chat(&Client::offline(), None, result) {
            Err(Error::GuardBotApprovalRequired { bot, query_id }) => assert_eq!((bot.id, query_id), (77, 5)),
            other => panic!("{other:?}"),
        }
    }
}
