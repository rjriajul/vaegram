use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::PeerRef;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotCommandScope {
    Default,
    AllPrivateChats,
    AllGroupChats,
    AllChatAdministrators,
    Chat(PeerRef),
    ChatAdministrators(PeerRef),
    ChatMember { chat_id: PeerRef, user_id: PeerRef },
}

impl BotCommandScope {
    pub(crate) async fn write(self, client: &Client) -> Result<base::BotCommandScope> {
        Ok(match self {
            BotCommandScope::Default => base::BotCommandScope::BotCommandScopeDefault,
            BotCommandScope::AllPrivateChats => base::BotCommandScope::BotCommandScopeUsers,
            BotCommandScope::AllGroupChats => base::BotCommandScope::BotCommandScopeChats,
            BotCommandScope::AllChatAdministrators => base::BotCommandScope::BotCommandScopeChatAdmins,
            BotCommandScope::Chat(chat_id) => {
                let peer = client.resolve_peer(chat_id).await?;
                base::BotCommandScope::BotCommandScopePeer(Box::new(types::BotCommandScopePeer { peer }))
            }
            BotCommandScope::ChatAdministrators(chat_id) => {
                let peer = client.resolve_peer(chat_id).await?;
                base::BotCommandScope::BotCommandScopePeerAdmins(Box::new(types::BotCommandScopePeerAdmins { peer }))
            }
            BotCommandScope::ChatMember { chat_id, user_id } => {
                let scope = types::BotCommandScopePeerUser {
                    peer: client.resolve_peer(chat_id).await?,
                    user_id: client.resolve_user(&user_id).await?,
                };
                base::BotCommandScope::BotCommandScopePeerUser(Box::new(scope))
            }
        })
    }
}
