use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::Chat;
use crate::utils::{get_channel_id, get_peer};

impl Client {
    pub async fn create_channel(&self, title: impl Into<String>, description: impl Into<String>) -> Result<Chat> {
        create(self, title.into(), description.into(), true).await
    }
}

pub(super) async fn create(client: &Client, title: String, about: String, broadcast: bool) -> Result<Chat> {
    let request = functions::channels::CreateChannel {
        broadcast,
        megagroup: !broadcast,
        for_import: false,
        forum: false,
        title,
        about,
        geo_point: None,
        address: None,
        ttl_period: None,
    };
    created_chat(client, client.invoke(&request).await?)
}

pub(super) fn created_chat(client: &Client, updates: base::Updates) -> Result<Chat> {
    let (users, chats) = match updates {
        base::Updates::Updates(u) => (u.users, u.chats),
        base::Updates::UpdatesCombined(u) => (u.users, u.chats),
        _ => Default::default(),
    };
    let id = chats
        .iter()
        .find_map(|c| match c {
            base::Chat::Chat(c) => Some(-c.id),
            base::Chat::Channel(c) => Some(get_channel_id(c.id)),
            _ => None,
        })
        .ok_or(Error::Unexpected("no chat in the updates"))?;
    let entities = Entities::new(client, users, chats);
    Ok(Chat::parse(&get_peer(id), &entities, false))
}
