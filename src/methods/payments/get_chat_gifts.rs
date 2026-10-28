use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{OwnedGift, OwnedGifts, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatGifts<'a> {
    client: &'a Client,
    chat_id: Option<PeerRef>,
    connection_id: Option<String>,
    request: functions::payments::GetSavedStarGifts,
}

impl Client {
    pub fn get_chat_gifts(&self, chat_id: impl Into<PeerRef>) -> GetChatGifts<'_> {
        GetChatGifts::new(self, Some(chat_id.into()), None)
    }
}

impl<'a> GetChatGifts<'a> {
    pub(crate) fn new(client: &'a Client, chat_id: Option<PeerRef>, connection_id: Option<String>) -> Self {
        let request = functions::payments::GetSavedStarGifts {
            exclude_unsaved: false,
            exclude_saved: false,
            exclude_unlimited: false,
            exclude_unique: false,
            sort_by_value: false,
            exclude_upgradable: false,
            exclude_unupgradable: false,
            peer_color_available: false,
            exclude_hosted: false,
            peer: base::InputPeer::InputPeerSelf,
            collection_id: None,
            offset: String::new(),
            limit: 100,
        };
        GetChatGifts { client, chat_id, connection_id, request }
    }

    pub fn exclude_unsaved(mut self) -> Self {
        self.request.exclude_unsaved = true;
        self
    }

    pub fn exclude_saved(mut self) -> Self {
        self.request.exclude_saved = true;
        self
    }

    pub fn exclude_unlimited(mut self) -> Self {
        self.request.exclude_unlimited = true;
        self
    }

    pub fn exclude_limited_upgradable(mut self) -> Self {
        self.request.exclude_upgradable = true;
        self
    }

    pub fn exclude_limited_non_upgradable(mut self) -> Self {
        self.request.exclude_unupgradable = true;
        self
    }

    pub fn exclude_unique(mut self) -> Self {
        self.request.exclude_unique = true;
        self
    }

    pub fn sort_by_price(mut self) -> Self {
        self.request.sort_by_value = true;
        self
    }

    pub fn offset(mut self, offset: impl Into<String>) -> Self {
        self.request.offset = offset.into();
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.request.limit = limit;
        self
    }

    async fn send(mut self) -> Result<OwnedGifts> {
        let client = self.client;
        // No chat leaves InputPeerSelf, which through a business connection is the business account.
        let owner_id = match &self.chat_id {
            Some(chat) => {
                let (id, peer) = client.resolve(chat).await?;
                self.request.peer = peer;
                id
            }
            None => 0,
        };
        let (base::payments::SavedStarGifts::SavedStarGifts(page), _) =
            client.invoke_business(self.connection_id.as_deref(), self.request).await?;
        let entities = Entities::new(client, page.users, page.chats);
        Ok(OwnedGifts {
            total_count: page.count,
            gifts: page
                .gifts
                .iter()
                .filter_map(|g| match g {
                    base::SavedStarGift::SavedStarGift(g) => OwnedGift::parse(g, owner_id, &entities),
                })
                .collect(),
            next_offset: page.next_offset,
        })
    }
}

into_future!(GetChatGifts, OwnedGifts);
