use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, User};

#[derive(Debug, Clone)]
pub struct PollAnswer {
    pub poll_id: i64,
    pub user: Option<User>,
    pub voter_chat: Option<Chat>,
    pub option_ids: Vec<i32>,
    client: Client,
}

impl PollAnswer {
    pub(crate) fn parse(client: &Client, raw: types::UpdateMessagePollVote, entities: &Entities) -> PollAnswer {
        let (user, voter_chat) = match &raw.peer {
            base::Peer::PeerUser(p) => (Some(User::parse(p.user_id, entities)), None),
            peer => (None, Some(Chat::parse(peer, entities, false))),
        };
        PollAnswer { poll_id: raw.poll_id, user, voter_chat, option_ids: raw.positions, client: client.clone() }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::PollAnswer;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    #[test]
    fn parses_user_and_chat_votes() {
        let raw = types::UpdateMessagePollVote {
            poll_id: 9,
            peer: base::Peer::PeerUser(Box::new(types::PeerUser { user_id: 7 })),
            options: vec![vec![0], vec![2]],
            positions: vec![0, 2],
            qts: 1,
        };
        let answer = PollAnswer::parse(&Client::offline(), raw.clone(), &Entities::default());
        assert_eq!((answer.poll_id, answer.user.map(|u| u.id), answer.voter_chat), (9, Some(7), None));
        assert_eq!(answer.option_ids, [0, 2]);

        let peer = base::Peer::PeerChannel(Box::new(types::PeerChannel { channel_id: 42 }));
        let raw = types::UpdateMessagePollVote { peer, options: Vec::new(), positions: Vec::new(), ..raw };
        let answer = PollAnswer::parse(&Client::offline(), raw, &Entities::default());
        assert_eq!((answer.user, answer.voter_chat.map(|c| c.id)), (None, Some(-1000000000042)));
        assert!(answer.option_ids.is_empty());
    }
}
