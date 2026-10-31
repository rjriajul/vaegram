use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::TopChatCategory;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::Chat;

impl Client {
    pub async fn get_top_chats(&self, category: TopChatCategory, limit: i32) -> Result<Vec<Chat>> {
        use TopChatCategory as C;
        if limit < 1 {
            return Err(Error::InvalidArgument("get_top_chats needs a limit of at least 1"));
        }
        let request = functions::contacts::GetTopPeers {
            correspondents: category == C::Users,
            bots_pm: category == C::Bots,
            bots_inline: category == C::InlineBots,
            phone_calls: category == C::Calls,
            // Forwarding targets are split into users and chats, and the category covers both.
            forward_users: category == C::ForwardChats,
            forward_chats: category == C::ForwardChats,
            groups: category == C::Groups,
            channels: category == C::Channels,
            bots_app: category == C::WebAppBots,
            bots_guestchat: category == C::GuestBots,
            offset: 0,
            limit,
            hash: 0,
        };
        let top = match self.invoke(&request).await? {
            base::contacts::TopPeers::TopPeers(top) => *top,
            // Disabled means the user switched suggestions off, so there are none.
            base::contacts::TopPeers::TopPeersDisabled | base::contacts::TopPeers::TopPeersNotModified => {
                return Ok(Vec::new());
            }
        };
        let entities = Entities::new(self, top.users, top.chats);
        let mut peers: Vec<types::TopPeer> = top
            .categories
            .into_iter()
            .flat_map(|base::TopPeerCategoryPeers::TopPeerCategoryPeers(c)| c.peers)
            .map(|base::TopPeer::TopPeer(p)| *p)
            .collect();
        peers.sort_by(|a, b| b.rating.total_cmp(&a.rating));
        Ok(peers.iter().map(|p| Chat::parse(&p.peer, &entities, false)).collect())
    }
}
