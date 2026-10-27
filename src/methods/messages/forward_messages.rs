use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Message;

impl Client {
    pub async fn forward_messages(&self, chat_id: i64, from_chat_id: i64, message_ids: &[i32]) -> Result<Vec<Message>> {
        let to_peer = self.resolve_peer(chat_id).await?;
        let from_peer = self.resolve_peer(from_chat_id).await?;
        let updates = self.invoke(&request(from_peer, to_peer, message_ids)).await?;
        Ok(Message::all_from_updates(self, updates).collect())
    }
}

fn request(from_peer: base::InputPeer, to_peer: base::InputPeer, ids: &[i32]) -> functions::messages::ForwardMessages {
    functions::messages::ForwardMessages {
        silent: false,
        background: false,
        with_my_score: false,
        drop_author: false,
        drop_media_captions: false,
        noforwards: false,
        allow_paid_floodskip: false,
        from_ephemeral: false,
        from_peer,
        id: ids.to_vec(),
        // One random_id per message, so Telegram can drop duplicates when the request is resent.
        random_id: ids.iter().map(|_| i64::from_le_bytes(random())).collect(),
        to_peer,
        top_msg_id: None,
        reply_to: None,
        schedule_date: None,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: None,
        video_timestamp: None,
        allow_paid_stars: None,
        suggested_post: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{base, request};

    #[test]
    fn one_random_id_per_message() {
        let ids = [3, 1, 2];
        let request = request(base::InputPeer::InputPeerSelf, base::InputPeer::InputPeerEmpty, &ids);
        assert_eq!((request.id.as_slice(), request.random_id.len()), (&ids[..], ids.len()));
        assert!(request.random_id.iter().enumerate().all(|(i, r)| !request.random_id[..i].contains(r)));
    }
}
