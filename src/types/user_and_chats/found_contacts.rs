use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::Chat;

/// The answer of a contact search: the account's own contacts and chats apart from the global results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FoundContacts {
    /// Matches among contacts and recent chats.
    pub my_results: Vec<Chat>,
    /// Matches among all public chats and users.
    pub global_results: Vec<Chat>,
}

impl FoundContacts {
    pub(crate) fn parse(client: &crate::Client, found: base::contacts::Found) -> FoundContacts {
        let base::contacts::Found::Found(found) = found;
        let entities = Entities::new(client, found.users, found.chats);
        let chats = |peers: &[base::Peer]| peers.iter().map(|peer| Chat::parse(peer, &entities, false)).collect();
        FoundContacts { my_results: chats(&found.my_results), global_results: chats(&found.results) }
    }
}
