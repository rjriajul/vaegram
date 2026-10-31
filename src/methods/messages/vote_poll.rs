use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    /// Votes in a poll for the options at the given positions (several only in multiple-choice polls).
    pub async fn vote_poll(&self, chat_id: impl Into<PeerRef>, message_id: i32, options: &[usize]) -> Result<()> {
        if options.is_empty() {
            return Err(Error::InvalidArgument("a vote takes at least one option; retract_vote takes a vote back"));
        }
        let chat_id = self.peer_id(chat_id.into()).await?;
        let options = self.poll_option_ids(chat_id, message_id, options).await?;
        let request =
            functions::messages::SendVote { peer: self.resolve_peer(chat_id).await?, msg_id: message_id, options };
        self.invoke(&request).await.map(drop)
    }

    // Poll option ids only live on the message, so naming options by position has to fetch it first.
    pub(super) async fn poll_option_ids(
        &self,
        chat_id: i64,
        message_id: i32,
        positions: &[usize],
    ) -> Result<Vec<Vec<u8>>> {
        let (mut found, _) = self.get_raw_messages(chat_id, &[message_id]).await?;
        let message = found.pop().flatten().ok_or(Error::InvalidArgument("no message with this id"))?;
        let Some(base::MessageMedia::MessageMediaPoll(media)) = message.media else {
            return Err(Error::InvalidArgument("this message has no poll"));
        };
        let base::Poll::Poll(poll) = media.poll;
        positions
            .iter()
            .map(|&i| match poll.answers.get(i) {
                Some(base::PollAnswer::PollAnswer(a)) => Ok(a.option.clone()),
                _ => Err(Error::InvalidArgument("the poll has no option at this position")),
            })
            .collect()
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
    use crate::types::{InputPollMedia, InputPollOption, Message};
    use crate::utils::get_peer;

    #[tokio::test]
    async fn an_empty_vote_is_refused_without_a_request() {
        let results = types::PollResults {
            min: false,
            has_unread_votes: false,
            can_view_stats: false,
            results: None,
            total_voters: None,
            recent_voters: None,
            solution: None,
            solution_entities: None,
            solution_media: None,
        };
        let media = types::MessageMediaPoll {
            poll: base::Poll::Poll(Box::new(poll(9, "Lunch?".into(), Vec::new()))),
            results: base::PollResults::PollResults(Box::new(results)),
            attached_media: None,
        };
        let mut raw = Message::bare(5, get_peer(7), 0);
        raw.media = Some(base::MessageMedia::MessageMediaPoll(Box::new(media)));
        let found = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(raw))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = base::messages::Messages::Messages(Box::new(found)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let peer = types::InputPeerUser { user_id: 7, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(peer)));

        assert!(matches!(client.vote_poll(7, 5, &[]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }

    fn user() -> base::InputPeer {
        base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }))
    }

    fn setup(replies: Vec<Vec<u8>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies.into_iter().map(Ok).collect());
        client.0.peers.lock().unwrap().insert(7, user());
        (client, rx)
    }

    fn answer(option: &[u8]) -> base::PollAnswer {
        let text = text_with_entities(String::from_utf8_lossy(option).into_owned());
        let answer = types::PollAnswer { text, option: option.to_vec(), media: None, added_by: None, date: None };
        base::PollAnswer::PollAnswer(Box::new(answer))
    }

    // Message 5 in the chat with user 7, holding a poll with these answers, or no poll at all.
    fn found(answers: Option<Vec<base::PollAnswer>>) -> Vec<u8> {
        let mut raw = Message::bare(5, get_peer(7), 0);
        raw.media = answers.map(|answers| {
            let results = types::PollResults {
                min: false,
                has_unread_votes: false,
                can_view_stats: false,
                results: None,
                total_voters: None,
                recent_voters: None,
                solution: None,
                solution_entities: None,
                solution_media: None,
            };
            let media = types::MessageMediaPoll {
                poll: base::Poll::Poll(Box::new(poll(9, "Lunch?".into(), answers))),
                results: base::PollResults::PollResults(Box::new(results)),
                attached_media: None,
            };
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

    fn updates() -> Vec<u8> {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    #[tokio::test]
    async fn votes_and_deletes_with_the_fetched_option_bytes() {
        let lunch = || found(Some(vec![answer(b"0"), answer(b"1")]));
        let (client, mut rx) = setup(vec![lunch(), updates(), lunch(), updates()]);

        client.vote_poll(7, 5, &[1]).await.unwrap();
        rx.try_recv().unwrap();
        let vote = messages::SendVote { peer: user(), msg_id: 5, options: vec![b"1".to_vec()] };
        assert_eq!(rx.try_recv().unwrap(), vote.to_bytes());

        client.delete_poll_option(7, 5, 1).await.unwrap();
        rx.try_recv().unwrap();
        let delete = messages::DeletePollAnswer { peer: user(), msg_id: 5, option: b"1".to_vec() };
        assert_eq!(rx.try_recv().unwrap(), delete.to_bytes());
    }

    #[tokio::test]
    async fn options_the_poll_lacks_are_refused_after_the_fetch() {
        let echo = types::InputPollAnswer { text: text_with_entities("Tea".into()), media: None };
        let echo = found(Some(vec![base::PollAnswer::InputPollAnswer(Box::new(echo))]));
        let (client, mut rx) = setup(vec![found(Some(vec![answer(b"0"), answer(b"1")])), found(None), echo]);
        let refused = [
            client.delete_poll_option(7, 5, 5).await,
            client.delete_poll_option(7, 5, 0).await,
            client.delete_poll_option(7, 5, 0).await,
        ];
        for result in refused {
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        let sent: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).map(|b| b[..4].to_vec()).collect();
        assert_eq!(sent, vec![messages::GetMessages::CONSTRUCTOR_ID.to_le_bytes().to_vec(); 3]);
    }

    #[tokio::test]
    async fn adds_an_option_with_its_media() {
        let (client, mut rx) = setup(vec![updates(), updates()]);
        let added = |media| {
            let answer = types::InputPollAnswer { text: text_with_entities("Tea".into()), media };
            let answer = base::PollAnswer::InputPollAnswer(Box::new(answer));
            messages::AddPollAnswer { peer: user(), msg_id: 5, answer }.to_bytes()
        };

        client.add_poll_option(7, 5, "Tea").await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), added(None));

        let option = InputPollOption::new("Tea").media(InputPollMedia::location(1.5, 2.5));
        client.add_poll_option(7, 5, option).await.unwrap();
        let point = types::InputGeoPoint { lat: 1.5, long: 2.5, accuracy_radius: None };
        let geo_point = base::InputGeoPoint::InputGeoPoint(Box::new(point));
        let media = base::InputMedia::InputMediaGeoPoint(Box::new(types::InputMediaGeoPoint { geo_point }));
        assert_eq!(rx.try_recv().unwrap(), added(Some(media)));
        assert!(rx.try_recv().is_err());
    }
}
