use super::forward_messages::request;
use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::{Error, Result};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct ForwardMediaGroup<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    message_id: i32,
    hide_sender_name: bool,
    hide_captions: bool,
    options: SendOptions,
}

impl Client {
    /// Forwards the whole album that `message_id` belongs to.
    pub fn forward_media_group(
        &self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        message_id: i32,
    ) -> ForwardMediaGroup<'_> {
        ForwardMediaGroup {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_id,
            hide_sender_name: false,
            hide_captions: false,
            options: SendOptions::default(),
        }
    }
}

impl ForwardMediaGroup<'_> {
    /// Leaves out the "Forwarded from" header.
    pub fn hide_sender_name(mut self) -> Self {
        self.hide_sender_name = true;
        self
    }

    pub fn hide_captions(mut self) -> Self {
        self.hide_captions = true;
        self
    }

    async fn send(self) -> Result<Vec<Message>> {
        if self.options.suggested_post_parameters.is_some() {
            return Err(Error::InvalidArgument("an album can't be a suggested post"));
        }
        let client = self.client;
        let (from_id, from_peer) = client.resolve(&self.from_chat_id).await?;
        let (group, _) = client.media_group(from_id, self.message_id).await?;
        let ids: Vec<i32> = group.iter().map(|m| m.id).collect();
        let (chat_id, to_peer) = client.resolve(&self.chat_id).await?;
        // Forwards name the topic in top_msg_id, so the reply carries none.
        let reply_to = self.options.input_reply_to(client, chat_id, None).await?;
        let mut request = request(from_peer, to_peer, &ids, reply_to, self.options);
        (request.drop_author, request.drop_media_captions) = (self.hide_sender_name, self.hide_captions);
        let mut messages: Vec<_> = Message::all_from_updates(client, client.invoke(&request).await?).collect();
        messages.sort_unstable_by_key(|m| m.id);
        Ok(messages)
    }
}

send_builder!(ForwardMediaGroup, Vec<Message>);
