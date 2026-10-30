use crate::raw::base;
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollVote {
    pub voter_id: i64,
    pub options: Vec<Vec<u8>>,
    pub date: i32,
}

impl PollVote {
    // With an option filter Telegram sends the reduced constructor, which leaves out the option it was asked about.
    pub(crate) fn parse(raw: base::MessagePeerVote, queried: Option<&[u8]>) -> PollVote {
        let (peer, options, date) = match raw {
            base::MessagePeerVote::MessagePeerVote(v) => (v.peer, vec![v.option], v.date),
            base::MessagePeerVote::MessagePeerVoteInputOption(v) => {
                (v.peer, queried.into_iter().map(<[u8]>::to_vec).collect(), v.date)
            }
            base::MessagePeerVote::MessagePeerVoteMultiple(v) => (v.peer, v.options, v.date),
        };
        PollVote { voter_id: get_peer_id(&peer), options, date }
    }
}

#[cfg(test)]
mod tests {
    use super::PollVote;
    use crate::raw::{base, types};

    #[test]
    fn keeps_every_chosen_option() {
        let peer = || base::Peer::PeerUser(types::PeerUser { user_id: 7 });
        let one = types::MessagePeerVote { peer: peer(), option: vec![1], date: 10 };
        let many = types::MessagePeerVoteMultiple { peer: peer(), options: vec![vec![0], vec![2]], date: 11 };
        let own = types::MessagePeerVoteInputOption { peer: peer(), date: 12 };
        let votes = [
            base::MessagePeerVote::MessagePeerVote(Box::new(one)),
            base::MessagePeerVote::MessagePeerVoteMultiple(Box::new(many)),
            base::MessagePeerVote::MessagePeerVoteInputOption(Box::new(own)),
        ];
        let votes = votes.map(|v| PollVote::parse(v, Some(&[2]))).map(|v| (v.voter_id, v.options, v.date));
        assert_eq!(votes, [(7, vec![vec![1]], 10), (7, vec![vec![0], vec![2]], 11), (7, vec![vec![2]], 12)]);
    }
}
