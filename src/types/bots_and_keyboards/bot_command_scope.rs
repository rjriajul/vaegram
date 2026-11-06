use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::PeerRef;

/// Where a set of bot commands applies.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BotCommandScope {
    /// Used when no narrower scope matches.
    Default,
    /// Every private chat with the bot.
    AllPrivateChats,
    /// Every group and supergroup the bot is in.
    AllGroupChats,
    /// Administrators of every group and supergroup the bot is in.
    AllChatAdministrators,
    /// One chat.
    Chat(PeerRef),
    /// The administrators of one chat.
    ChatAdministrators(PeerRef),
    /// One user in one chat.
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
