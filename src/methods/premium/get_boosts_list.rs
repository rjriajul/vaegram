use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::{ChatBoost, PeerRef};

#[must_use = "call .next() to fetch boosts"]
pub struct GetBoostsList<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    gifts: bool,
    pages: OffsetPages<ChatBoost>,
}

impl Client {
    pub fn get_boosts_list(&self, chat_id: impl Into<PeerRef>) -> GetBoostsList<'_> {
        GetBoostsList { client: self, chat_id: chat_id.into(), gifts: false, pages: OffsetPages::new(100) }
    }
}

impl GetBoostsList<'_> {
    pub fn gifts(mut self) -> Self {
        self.gifts = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub async fn next(&mut self) -> Result<Option<ChatBoost>> {
        loop {
            if let Some(boost) = self.pages.pop() {
                return Ok(Some(boost));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let offset = self.pages.next_offset().unwrap_or_default().into();
            let request = functions::premium::GetBoostsList { gifts: self.gifts, peer, offset, limit };
            let base::premium::BoostsList::BoostsList(page) = self.client.invoke(&request).await?;
            let entities = Entities::new(self.client, page.users, Vec::new());
            let boosts =
                |raw: Vec<base::Boost>| raw.into_iter().filter_map(|b| ChatBoost::parse(b, &entities)).collect();
            self.pages.refill(page.boosts, page.next_offset, boosts);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::utils::get_channel_id;

    fn page(ids: &[&str], next_offset: Option<&str>) -> Vec<u8> {
        let boost = |id: &&str| {
            base::Boost::Boost(Box::new(types::Boost {
                gift: false,
                giveaway: false,
                unclaimed: false,
                id: (*id).into(),
                user_id: Some(5),
                giveaway_msg_id: None,
                date: 1,
                expires: 2,
                used_gift_slug: None,
                multiplier: None,
                stars: None,
            }))
        };
        let list = types::premium::BoostsList {
            count: 3,
            boosts: ids.iter().map(boost).collect(),
            next_offset: next_offset.map(Into::into),
            users: Vec::new(),
        };
        base::premium::BoostsList::BoostsList(Box::new(list)).to_bytes()
    }

    #[tokio::test]
    async fn walks_pages_until_the_offset_runs_out() {
        let (client, mut sent) = Client::scripted(vec![Ok(page(&["a", "b"], Some("next"))), Ok(page(&["c"], None))]);
        let peer = types::InputPeerChannel { channel_id: 1, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(get_channel_id(1), base::InputPeer::InputPeerChannel(Box::new(peer)));

        let mut boosts = client.get_boosts_list(get_channel_id(1));
        let mut ids = Vec::new();
        while let Some(boost) = boosts.next().await.unwrap() {
            ids.push(boost.boost_id);
        }
        assert_eq!(ids, ["a", "b", "c"]);
        assert!(sent.try_recv().is_ok() && sent.try_recv().is_ok() && sent.try_recv().is_err());
    }
}
