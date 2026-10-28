use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatJoiner, PeerRef};

const IMPORTERS_LIMIT: usize = 100;

#[must_use = "a request does nothing until it is awaited"]
pub struct GetChatJoinRequests<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query: Option<String>,
    limit: usize,
}

impl Client {
    /// Fetches every page before returning; `limit` caps it. User accounts only.
    pub fn get_chat_join_requests(&self, chat_id: impl Into<PeerRef>) -> GetChatJoinRequests<'_> {
        GetChatJoinRequests { client: self, chat_id: chat_id.into(), query: None, limit: usize::MAX }
    }
}

impl GetChatJoinRequests<'_> {
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    async fn send(self) -> Result<Vec<ChatJoiner>> {
        let client = self.client;
        let mut request = functions::messages::GetChatInviteImporters {
            requested: true,
            subscription_expired: false,
            peer: client.resolve_peer(self.chat_id).await?,
            link: None,
            q: self.query,
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit: 0,
        };
        let mut joiners = Vec::new();
        while joiners.len() < self.limit {
            request.limit = IMPORTERS_LIMIT.min(self.limit - joiners.len()) as i32;
            let base::messages::ChatInviteImporters::ChatInviteImporters(page) = client.invoke(&request).await?;
            let page = *page;
            let Some(base::ChatInviteImporter::ChatInviteImporter(last)) = page.importers.last() else { break };
            let (offset_date, offset_user) = (last.date, last.user_id);
            let entities = Entities::new(client, page.users, Vec::new());
            joiners.extend(
                page.importers
                    .into_iter()
                    .map(|base::ChatInviteImporter::ChatInviteImporter(i)| ChatJoiner::parse(*i, &entities)),
            );
            if joiners.len() >= page.count as usize {
                break;
            }
            // The next page starts after this (date, user) pair; the user's access hash was just cached.
            request.offset_date = offset_date;
            request.offset_user = client.resolve_user(&offset_user.into()).await?;
        }
        Ok(joiners)
    }
}

into_future!(GetChatJoinRequests, Vec<ChatJoiner>);

#[cfg(test)]
mod tests {
    use super::super::create_chat_invite_link::tests::{CHANNEL_ID, channel, setup};
    use crate::raw::functions::messages;
    use crate::raw::{Deserializable, Serializable, base, types};
    use crate::utils::get_channel_id;

    fn user(id: i64) -> base::User {
        let mut bytes = Vec::new();
        // user#b1b8cc83, flags with only access_hash set, empty flags2; every other field is optional.
        for word in [0xb1b8cc83, 1, 0] {
            bytes.extend(u32::to_le_bytes(word));
        }
        bytes.extend(id.to_le_bytes());
        bytes.extend((id * 10).to_le_bytes());
        base::User::from_bytes(&bytes).unwrap()
    }

    fn page(count: i32, joiners: &[(i64, i32)]) -> Vec<u8> {
        let (mut importers, mut users) = (Vec::new(), Vec::new());
        for &(user_id, date) in joiners {
            let importer = types::ChatInviteImporter {
                requested: true,
                via_chatlist: false,
                user_id,
                date,
                about: None,
                approved_by: None,
            };
            importers.push(base::ChatInviteImporter::ChatInviteImporter(Box::new(importer)));
            users.push(user(user_id));
        }
        let page = types::messages::ChatInviteImporters { count, importers, users };
        base::messages::ChatInviteImporters::ChatInviteImporters(Box::new(page)).to_bytes()
    }

    #[tokio::test]
    async fn pages_join_requests_after_last_joiner() {
        let (client, mut rx) = setup(vec![page(3, &[(10, 300), (11, 200)]), page(3, &[(12, 100)])]);
        let joiners = client.get_chat_join_requests(get_channel_id(CHANNEL_ID)).query("a").await.unwrap();
        let joiners: Vec<_> = joiners.iter().map(|j| (j.user.id, j.date)).collect();
        assert_eq!(joiners, [(10, 300), (11, 200), (12, 100)]);

        let mut request = messages::GetChatInviteImporters {
            requested: true,
            subscription_expired: false,
            peer: channel(),
            link: None,
            q: Some("a".into()),
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit: 100,
        };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        request.offset_date = 200;
        request.offset_user = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 11, access_hash: 110 }));
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert!(rx.try_recv().is_err());
    }
}
