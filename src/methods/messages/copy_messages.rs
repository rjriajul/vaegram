use super::forward_messages::request;
use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::base;
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct CopyMessages<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    message_ids: &'a [i32],
    remove_caption: bool,
    options: SendOptions,
}

impl Client {
    pub fn copy_messages<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        message_ids: &'a [i32],
    ) -> CopyMessages<'a> {
        CopyMessages {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            message_ids,
            remove_caption: false,
            options: SendOptions::default(),
        }
    }
}

impl CopyMessages<'_> {
    pub fn remove_caption(mut self) -> Self {
        self.remove_caption = true;
        self
    }

    async fn send(self) -> Result<Vec<i32>> {
        if !(1..=100).contains(&self.message_ids.len()) {
            return Err(Error::InvalidArgument("copies take 1 to 100 message ids"));
        }
        let client = self.client;
        let (chat_id, to_peer) = client.resolve(&self.chat_id).await?;
        let from_peer = client.resolve(&self.from_chat_id).await?.1;
        // Like forwards, the topic goes in top_msg_id, so the reply carries none.
        let reply_to = self.options.input_reply_to(client, chat_id, None).await?;
        let mut request = request(from_peer, to_peer, self.message_ids, reply_to, self.options);
        request.drop_author = true;
        request.drop_media_captions = self.remove_caption;
        let updates = client.invoke(&request).await?;
        Ok(sent_ids(&updates, &request.random_id))
    }
}

send_builder!(CopyMessages, Vec<i32>);

fn sent_ids(updates: &base::Updates, random_ids: &[i64]) -> Vec<i32> {
    let updates = match updates {
        base::Updates::Updates(u) => &u.updates,
        base::Updates::UpdatesCombined(u) => &u.updates,
        _ => return Vec::new(),
    };
    let sent_id = |r: &i64| {
        updates.iter().find_map(|u| match u {
            base::Update::UpdateMessageID(u) if u.random_id == *r => Some(u.id),
            _ => None,
        })
    };
    random_ids.iter().filter_map(sent_id).collect()
}

#[cfg(test)]
mod tests {
    use super::sent_ids;
    use crate::raw::{base, types};

    #[test]
    fn keeps_request_order_and_skips_missing() {
        let update = |random_id, id| base::Update::UpdateMessageID(Box::new(types::UpdateMessageID { id, random_id }));
        let updates = base::Updates::Updates(Box::new(types::Updates {
            updates: vec![update(30, 12), base::Update::UpdateConfig, update(10, 11)],
            users: Vec::new(),
            chats: Vec::new(),
            date: 0,
            seq: 0,
        }));
        assert_eq!(sent_ids(&updates, &[10, 20, 30]), [11, 12]);
        assert!(sent_ids(&base::Updates::UpdatesTooLong, &[10]).is_empty());
    }
}
