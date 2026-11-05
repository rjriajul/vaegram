use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{InlineQueryResult, PeerRef, PreparedInlineMessage};

/// Request built by [`Client::save_prepared_inline_message`]. Awaiting it returns the [`PreparedInlineMessage`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SavePreparedInlineMessage<'a> {
    client: &'a Client,
    user_id: PeerRef,
    result: InlineQueryResult,
    peer_types: Vec<base::InlineQueryPeerType>,
}

impl Client {
    /// Stores a message that `user_id` can share from the bot's Mini App.
    ///
    /// Without any `allow_*` option it can go to every chat type.
    pub fn save_prepared_inline_message(
        &self,
        user_id: impl Into<PeerRef>,
        result: impl Into<InlineQueryResult>,
    ) -> SavePreparedInlineMessage<'_> {
        let (user_id, result) = (user_id.into(), result.into());
        SavePreparedInlineMessage { client: self, user_id, result, peer_types: Vec::new() }
    }
}

impl SavePreparedInlineMessage<'_> {
    /// Lets the message be shared in private chats with users.
    pub fn allow_user_chats(mut self) -> Self {
        self.peer_types.push(base::InlineQueryPeerType::InlineQueryPeerTypePM);
        self
    }

    /// Lets the message be shared in private chats with bots.
    pub fn allow_bot_chats(mut self) -> Self {
        self.peer_types.push(base::InlineQueryPeerType::InlineQueryPeerTypeBotPM);
        self
    }

    /// Lets the message be shared in groups and supergroups.
    pub fn allow_group_chats(mut self) -> Self {
        self.peer_types.push(base::InlineQueryPeerType::InlineQueryPeerTypeChat);
        self.peer_types.push(base::InlineQueryPeerType::InlineQueryPeerTypeMegagroup);
        self
    }

    /// Lets the message be shared in channels.
    pub fn allow_channel_chats(mut self) -> Self {
        self.peer_types.push(base::InlineQueryPeerType::InlineQueryPeerTypeBroadcast);
        self
    }

    async fn send(self) -> Result<PreparedInlineMessage> {
        let client = self.client;
        let user_id = client.resolve_user(&self.user_id).await?;
        let request = functions::messages::SavePreparedInlineMessage {
            result: self.result.write(client).await?,
            user_id,
            peer_types: (!self.peer_types.is_empty()).then_some(self.peer_types),
        };
        let base::messages::BotPreparedInlineMessage::BotPreparedInlineMessage(m) = client.invoke(&request).await?;
        Ok(PreparedInlineMessage { id: m.id, expiration_date: m.expire_date })
    }
}

into_future!(SavePreparedInlineMessage, PreparedInlineMessage);
