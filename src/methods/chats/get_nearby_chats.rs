use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Chat, NearbyChat};

impl Client {
    /// Finds groups and channels near a location; people are left out.
    pub async fn get_nearby_chats(&self, latitude: f64, longitude: f64) -> Result<Vec<NearbyChat>> {
        let point = types::InputGeoPoint { lat: latitude, long: longitude, accuracy_radius: None };
        let request = functions::contacts::GetLocated {
            background: false,
            geo_point: base::InputGeoPoint::InputGeoPoint(Box::new(point)),
            self_expires: None,
        };
        Ok(nearby_chats(self, self.invoke(&request).await?))
    }
}

fn nearby_chats(client: &Client, updates: base::Updates) -> Vec<NearbyChat> {
    let (updates, users, chats) = match updates {
        base::Updates::Updates(u) => (u.updates, u.users, u.chats),
        base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
        base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
        _ => return Vec::new(),
    };
    let entities = Entities::new(client, users, chats);
    let mut found = Vec::new();
    for update in updates {
        let base::Update::UpdatePeerLocated(located) = update else { continue };
        for peer in &located.peers {
            // The other kind is the account's own broadcast location, not a chat.
            let base::PeerLocated::PeerLocated(peer) = peer else { continue };
            if matches!(peer.peer, base::Peer::PeerUser(_)) {
                continue;
            }
            found.push(NearbyChat { chat: Chat::parse(&peer.peer, &entities, false), distance: peer.distance });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::nearby_chats;
    use crate::Client;
    use crate::raw::{base, types};

    fn located(peer: base::Peer, distance: i32) -> base::PeerLocated {
        base::PeerLocated::PeerLocated(Box::new(types::PeerLocated { peer, expires: 0, distance }))
    }

    fn short(update: base::Update) -> base::Updates {
        base::Updates::UpdateShort(Box::new(types::UpdateShort { update, date: 0 }))
    }

    #[test]
    fn nearby_maps_distance_and_skips_users() {
        let user = base::Peer::PeerUser(types::PeerUser { user_id: 5 });
        let chat = base::Peer::PeerChat(types::PeerChat { chat_id: 7 });
        let update = base::Update::UpdatePeerLocated(Box::new(types::UpdatePeerLocated {
            peers: vec![located(user, 10), located(chat, 300)],
        }));
        let found = nearby_chats(&Client::offline(), short(update));
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].chat.id, found[0].distance), (-7, 300));
    }

    #[test]
    fn nearby_without_located_update_is_empty() {
        let update = base::Update::UpdateUserStatus(Box::new(types::UpdateUserStatus {
            user_id: 5,
            status: base::UserStatus::UserStatusEmpty,
        }));
        assert!(nearby_chats(&Client::offline(), short(update)).is_empty());
        assert!(nearby_chats(&Client::offline(), base::Updates::UpdatesTooLong).is_empty());
    }
}
