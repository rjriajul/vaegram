use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef, SuggestedPostParameters};

#[must_use = "a request does nothing until it is awaited"]
pub struct ForwardMessages<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    message_ids: &'a [i32],
    options: SendOptions,
}

impl Client {
    pub fn forward_messages<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        message_ids: &'a [i32],
    ) -> ForwardMessages<'a> {
        ForwardMessages {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_ids,
            options: SendOptions::default(),
        }
    }
}

impl ForwardMessages<'_> {
    async fn send(self) -> Result<Vec<Message>> {
        if !(1..=100).contains(&self.message_ids.len()) {
            return Err(Error::InvalidArgument("forwards take 1 to 100 message ids"));
        }
        let client = self.client;
        let (chat_id, to_peer) = client.resolve(&self.chat_id).await?;
        let from_peer = client.resolve(&self.from_chat_id).await?.1;
        // Forwards name the topic in top_msg_id, so the reply carries none.
        let reply_to = self.options.input_reply_to(client, chat_id, None).await?;
        let request = request(from_peer, to_peer, self.message_ids, reply_to, self.options);
        let updates = client.invoke(&request).await?;
        Ok(Message::all_from_updates(client, updates).collect())
    }
}

send_builder!(ForwardMessages, Vec<Message>);

pub(super) fn request(
    from_peer: base::InputPeer,
    to_peer: base::InputPeer,
    ids: &[i32],
    reply_to: Option<base::InputReplyTo>,
    options: SendOptions,
) -> functions::messages::ForwardMessages {
    functions::messages::ForwardMessages {
        silent: options.disable_notification,
        background: false,
        with_my_score: false,
        drop_author: false,
        drop_media_captions: false,
        noforwards: options.protect_content,
        allow_paid_floodskip: options.allow_paid_broadcast,
        from_ephemeral: false,
        from_peer,
        id: ids.to_vec(),
        // One random_id per message, so Telegram can drop duplicates when the request is resent.
        random_id: ids.iter().map(|_| i64::from_le_bytes(random())).collect(),
        to_peer,
        top_msg_id: options.thread(),
        reply_to,
        schedule_date: options.schedule_date,
        schedule_repeat_period: None,
        send_as: None,
        quick_reply_shortcut: None,
        effect: options.message_effect_id,
        video_timestamp: None,
        allow_paid_stars: options.paid_message_star_count,
        suggested_post: options.suggested_post_parameters.map(SuggestedPostParameters::write),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_random_id_per_message() {
        let ids = [3, 1, 2];
        let peers = (base::InputPeer::InputPeerSelf, base::InputPeer::InputPeerEmpty);
        let request = request(peers.0, peers.1, &ids, None, SendOptions::default());
        assert_eq!(request.id, ids);
        assert_eq!(request.random_id.len(), ids.len());
        assert!(request.random_id.iter().enumerate().all(|(i, r)| !request.random_id[..i].contains(r)));
    }

    #[test]
    fn options_map_to_forward_flags() {
        let options = SendOptions {
            disable_notification: true,
            protect_content: true,
            allow_paid_broadcast: true,
            schedule_date: Some(1_900_000_000),
            message_effect_id: Some(5),
            paid_message_star_count: Some(10),
            message_thread_id: Some(7),
            ..Default::default()
        };
        let r = request(base::InputPeer::InputPeerSelf, base::InputPeer::InputPeerEmpty, &[1], None, options);
        assert_eq!((r.silent, r.noforwards, r.allow_paid_floodskip, r.top_msg_id), (true, true, true, Some(7)));
        assert_eq!((r.schedule_date, r.effect, r.allow_paid_stars), (Some(1_900_000_000), Some(5), Some(10)));
    }

    #[test]
    fn forwards_to_general_name_no_topic() {
        let options = SendOptions { message_thread_id: Some(1), ..Default::default() };
        let r = request(base::InputPeer::InputPeerSelf, base::InputPeer::InputPeerEmpty, &[1], None, options);
        assert_eq!(r.top_msg_id, None);
    }
}
