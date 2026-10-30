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
        // Poll option ids only live on the message, so voting by position has to fetch it first.
        let (mut found, _) = self.get_raw_messages(chat_id, &[message_id]).await?;
        let message = found.pop().flatten().ok_or(Error::InvalidArgument("no message with this id"))?;
        let Some(base::MessageMedia::MessageMediaPoll(media)) = message.media else {
            return Err(Error::InvalidArgument("this message has no poll"));
        };
        let base::Poll::Poll(poll) = media.poll;
        let options = options
            .iter()
            .map(|&i| match poll.answers.get(i) {
                Some(base::PollAnswer::PollAnswer(a)) => Ok(a.option.clone()),
                _ => Err(Error::InvalidArgument("the poll has no option at this position")),
            })
            .collect::<Result<_>>()?;
        let request =
            functions::messages::SendVote { peer: self.resolve_peer(chat_id).await?, msg_id: message_id, options };
        self.invoke(&request).await.map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_poll::poll;
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, types};
    use crate::types::Message;
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
}
