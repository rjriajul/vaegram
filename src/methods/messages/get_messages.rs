use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef};
use crate::utils::{CHANNEL_OFFSET, get_peer_id};

impl Client {
    /// The result lines up with `message_ids`; missing or service messages are `None`.
    pub async fn get_messages(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<Vec<Option<Message>>> {
        let (found, entities) = self.get_raw_messages(chat_id, message_ids).await?;
        Ok(found.into_iter().map(|m| m.map(|m| Message::parse(self, m, &entities))).collect())
    }

    pub(crate) async fn get_raw_messages(
        &self,
        chat_id: impl Into<PeerRef>,
        message_ids: &[i32],
    ) -> Result<(Vec<Option<Box<types::Message>>>, Entities)> {
        let chat_id = self.peer_id(chat_id.into()).await?;
        let id = message_ids
            .iter()
            .map(|&id| base::InputMessage::InputMessageID(Box::new(types::InputMessageID { id })))
            .collect();
        let found = if chat_id <= -CHANNEL_OFFSET {
            let channel = self.resolve_channel(&chat_id.into()).await?;
            self.invoke(&functions::channels::GetMessages { channel, id }).await?
        } else {
            self.invoke(&functions::messages::GetMessages { id }).await?
        };
        let (found, entities) = Message::unpack(self, found);
        // Outside channels message ids are per account, so messages.getMessages can return other chats' messages.
        let mut found: Vec<_> = found
            .into_iter()
            .filter_map(|m| match m {
                base::Message::Message(m) if get_peer_id(&m.peer_id) == chat_id => Some(Some(m)),
                _ => None,
            })
            .collect();
        let messages = message_ids
            .iter()
            .map(|&id| found.iter_mut().find(|m| m.as_ref().is_some_and(|m| m.id == id))?.take())
            .collect();
        Ok((messages, entities))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::raw::functions::{channels, messages};
    use crate::raw::{Identifiable, base, types};
    use crate::utils::get_channel_id;

    #[tokio::test]
    async fn channels_use_channel_methods() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        let rx = &mut rx;

        let ids = (
            sent(rx, client.get_messages(-5, &[1])).await,
            sent(rx, client.get_messages(channel, &[1])).await,
            sent(rx, client.read_chat_history(-5, 0)).await,
            sent(rx, client.read_chat_history(channel, 0)).await,
        );
        let expected = (
            messages::GetMessages::CONSTRUCTOR_ID,
            channels::GetMessages::CONSTRUCTOR_ID,
            messages::ReadHistory::CONSTRUCTOR_ID,
            channels::ReadHistory::CONSTRUCTOR_ID,
        );
        assert_eq!(ids, expected);
    }
}
