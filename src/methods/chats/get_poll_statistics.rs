use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, PollStatistics};

impl Client {
    pub async fn get_poll_statistics(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<PollStatistics> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::stats::GetPollStats { dark: false, peer, msg_id: message_id };
        Ok(PollStatistics::parse(self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{PollStatistics, StatisticalGraph};
    use crate::utils::get_channel_id;

    #[tokio::test]
    async fn poll_statistics_sends_and_maps() {
        let graph = base::StatsGraph::StatsGraphAsync(Box::new(types::StatsGraphAsync { token: "t".into() }));
        let reply = base::stats::PollStats::PollStats(Box::new(types::stats::PollStats { votes_graph: graph }));
        let (client, mut rx) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let peer = base::InputPeer::InputPeerChannel(Box::new(peer));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), peer.clone());

        let statistics = client.get_poll_statistics(get_channel_id(9), 5).await.unwrap();
        assert_eq!(statistics, PollStatistics { votes_graph: StatisticalGraph::Async { token: "t".into() } });
        let request = functions::stats::GetPollStats { dark: false, peer, msg_id: 5 };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }
}
