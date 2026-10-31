use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, Poll};

impl Client {
    pub async fn get_poll_results(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Poll> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let request = functions::messages::GetPollResults { peer, msg_id: message_id, poll_hash: 0 };
        let (updates, users, chats) = match self.invoke(&request).await? {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => Default::default(),
        };
        let entities = Entities::new(self, users, chats);
        let update = updates.into_iter().find_map(|u| match u {
            base::Update::UpdateMessagePoll(u) => Some(*u),
            _ => None,
        });
        let results = match update {
            Some(types::UpdateMessagePoll { poll: Some(poll), results, .. }) => {
                return Ok(Poll::parse(&poll, &results, &entities));
            }
            update => update.map(|u| u.results),
        };
        // The poll itself is optional in the update, so it then comes from the message, with the update's counts.
        let (found, entities) = self.get_raw_messages(chat_id, &[message_id]).await?;
        let media = found
            .into_iter()
            .flatten()
            .find_map(|m| match m.media {
                Some(base::MessageMedia::MessageMediaPoll(media)) => Some(media),
                _ => None,
            })
            .ok_or(Error::InvalidArgument("no poll at that message id"))?;
        let results = results.unwrap_or(media.results);
        Ok(Poll::parse(&media.poll, &results, &entities))
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use super::super::send_poll::{poll, text_with_entities};
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

    fn setup(replies: Vec<Vec<u8>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies.into_iter().map(Ok).collect());
        client.0.peers.lock().unwrap().insert(7, user());
        (client, rx)
    }

    fn lunch() -> base::Poll {
        let answer = |option: &[u8]| {
            let text = text_with_entities(String::from_utf8_lossy(option).into_owned());
            let answer = types::PollAnswer { text, option: option.to_vec(), media: None, added_by: None, date: None };
            base::PollAnswer::PollAnswer(Box::new(answer))
        };
        base::Poll::Poll(Box::new(poll(9, "Lunch?".into(), vec![answer(b"0"), answer(b"1")])))
    }

    // Results with `votes` voters on option "1".
    fn results(votes: i32) -> base::PollResults {
        let voters = types::PollAnswerVoters {
            chosen: false,
            correct: false,
            option: b"1".to_vec(),
            voters: Some(votes),
            // voters and recent_voters share one flag, so both must be set for the reply to decode.
            recent_voters: Some(Vec::new()),
        };
        let results = types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: Some(vec![base::PollAnswerVoters::PollAnswerVoters(Box::new(voters))]),
            total_voters: Some(votes),
            recent_voters: None,
            solution: None,
            solution_entities: None,
            solution_media: None,
        };
        base::PollResults::PollResults(Box::new(results))
    }

    fn updated(poll: Option<base::Poll>, votes: Option<i32>) -> Vec<u8> {
        let updates = votes.map(|votes| {
            let update = types::UpdateMessagePoll {
                peer: Some(get_peer(7)),
                msg_id: Some(5),
                top_msg_id: None,
                poll_id: 9,
                poll,
                results: results(votes),
            };
            base::Update::UpdateMessagePoll(Box::new(update))
        });
        let updates = types::Updates {
            updates: updates.into_iter().collect(),
            users: Vec::new(),
            chats: Vec::new(),
            date: 1,
            seq: 0,
        };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    // Message 5 in the chat with user 7, with a poll that has `votes` voters on option "1", or no poll.
    fn found(votes: Option<i32>) -> Vec<u8> {
        let mut raw = Message::bare(5, get_peer(7), 0);
        raw.media = votes.map(|votes| {
            let media = types::MessageMediaPoll { poll: lunch(), results: results(votes), attached_media: None };
            base::MessageMedia::MessageMediaPoll(Box::new(media))
        });
        let found = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(raw))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Messages::Messages(Box::new(found)).to_bytes()
    }

    fn counts(poll: &crate::types::Poll) -> (&str, Vec<i32>) {
        (poll.question.as_str(), poll.options.iter().map(|o| o.voter_count).collect())
    }

    #[tokio::test]
    async fn results_come_from_the_poll_update() {
        let (client, mut rx) = setup(vec![updated(Some(lunch()), Some(3))]);
        let poll = client.get_poll_results(7, 5).await.unwrap();
        assert_eq!(counts(&poll), ("Lunch?", vec![0, 3]));
        let request = messages::GetPollResults { peer: user(), msg_id: 5, poll_hash: 0 };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_update_without_the_poll_reads_the_message() {
        let (client, mut rx) = setup(vec![updated(None, Some(3)), found(Some(1))]);
        let poll = client.get_poll_results(7, 5).await.unwrap();
        assert_eq!(counts(&poll), ("Lunch?", vec![0, 3]));
        rx.try_recv().unwrap();
        assert_eq!(rx.try_recv().unwrap()[..4], messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes());
    }

    #[tokio::test]
    async fn no_poll_update_uses_the_messages_own_results() {
        let (client, _rx) = setup(vec![updated(None, None), found(Some(1))]);
        let poll = client.get_poll_results(7, 5).await.unwrap();
        assert_eq!(counts(&poll), ("Lunch?", vec![0, 1]));
    }

    #[tokio::test]
    async fn a_message_without_a_poll_is_invalid_argument() {
        let (client, _rx) = setup(vec![updated(None, None), found(None)]);
        let result = client.get_poll_results(7, 5).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
    }
}
