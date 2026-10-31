use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

// The General topic of every forum has this fixed id.
pub(crate) const GENERAL_TOPIC_ID: i32 = 1;

pub struct EditForumTopic<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    topic_id: i32,
    title: Option<String>,
    icon_emoji_id: Option<i64>,
    closed: Option<bool>,
    hidden: Option<bool>,
}

impl Client {
    pub fn edit_forum_topic(&self, chat_id: impl Into<PeerRef>, topic_id: i32) -> EditForumTopic<'_> {
        let chat_id = chat_id.into();
        EditForumTopic { client: self, chat_id, topic_id, title: None, icon_emoji_id: None, closed: None, hidden: None }
    }
}

impl EditForumTopic<'_> {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// 0 removes the icon.
    pub fn icon_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.icon_emoji_id = Some(custom_emoji_id);
        self
    }

    pub(crate) fn closed(mut self, closed: bool) -> Self {
        self.closed = Some(closed);
        self
    }

    pub(crate) fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let request = functions::messages::EditForumTopic {
            peer: client.resolve_peer(self.chat_id).await?,
            topic_id: self.topic_id,
            title: self.title,
            icon_emoji_id: self.icon_emoji_id,
            closed: self.closed,
            hidden: self.hidden,
        };
        client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(EditForumTopic, ());

#[cfg(test)]
mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::ForumTopic;
    use crate::utils::{get_channel_id, get_peer};

    const USER: i64 = 7;

    fn topic_peer() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }))
    }

    fn topic_client(replies: Vec<Vec<u8>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies.into_iter().map(Ok).collect());
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }));
        client.0.peers.lock().unwrap().extend([(get_channel_id(9), topic_peer()), (USER, user)]);
        (client, rx)
    }

    async fn body<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    fn edit(topic_id: i32) -> messages::EditForumTopic {
        messages::EditForumTopic {
            peer: topic_peer(),
            topic_id,
            title: None,
            icon_emoji_id: None,
            closed: None,
            hidden: None,
        }
    }

    fn affected(offset: i32) -> Vec<u8> {
        let affected = types::messages::AffectedHistory { pts: 1, pts_count: 1, offset };
        base::messages::AffectedHistory::AffectedHistory(Box::new(affected)).to_bytes()
    }

    #[tokio::test]
    async fn builds_forum_topic_edits() {
        let (client, mut rx) = topic_client(Vec::new());
        let (rx, chat) = (&mut rx, get_channel_id(9));
        let renamed = messages::EditForumTopic { title: Some("t".into()), icon_emoji_id: Some(0), ..edit(5) };
        assert_eq!(body(rx, client.edit_forum_topic(chat, 5).title("t").icon_emoji_id(0)).await, renamed.to_bytes());
        let closed = |topic_id, closed| messages::EditForumTopic { closed: Some(closed), ..edit(topic_id) }.to_bytes();
        let hidden = |hidden| messages::EditForumTopic { hidden: Some(hidden), ..edit(1) }.to_bytes();
        assert_eq!(body(rx, client.close_forum_topic(chat, 5)).await, closed(5, true));
        assert_eq!(body(rx, client.reopen_forum_topic(chat, 5)).await, closed(5, false));
        let general = messages::EditForumTopic { title: Some("g".into()), ..edit(1) };
        assert_eq!(body(rx, client.edit_general_forum_topic(chat, "g")).await, general.to_bytes());
        assert_eq!(body(rx, client.close_general_forum_topic(chat)).await, closed(1, true));
        assert_eq!(body(rx, client.reopen_general_forum_topic(chat)).await, closed(1, false));
        assert_eq!(body(rx, client.hide_general_forum_topic(chat)).await, hidden(true));
        assert_eq!(body(rx, client.unhide_general_forum_topic(chat)).await, hidden(false));
        let icons = messages::GetStickerSet {
            stickerset: base::InputStickerSet::InputStickerSetEmojiDefaultTopicIcons,
            hash: 0,
        };
        assert_eq!(body(rx, client.get_forum_topic_icon_stickers()).await, icons.to_bytes());
    }

    #[tokio::test]
    async fn repeats_topic_batches_until_offset_is_zero() {
        let (client, mut rx) = topic_client(vec![affected(10), affected(0), affected(10), affected(0)]);
        client.delete_forum_topic(get_channel_id(9), 5).await.unwrap();
        let delete = messages::DeleteTopicHistory { peer: topic_peer(), top_msg_id: 5 }.to_bytes();
        assert_eq!((rx.try_recv().unwrap(), rx.try_recv().unwrap()), (delete.clone(), delete));

        client.unpin_all_general_forum_topic_messages(USER).await.unwrap();
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }));
        let unpin = messages::UnpinAllMessages { peer: user, top_msg_id: Some(1), saved_peer_id: None }.to_bytes();
        assert_eq!((rx.try_recv().unwrap(), rx.try_recv().unwrap()), (unpin.clone(), unpin));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn creates_topic_from_its_service_message() {
        let action = types::MessageActionTopicCreate {
            title_missing: false,
            title: "Ideas".into(),
            icon_color: 0x6FB9F0,
            icon_emoji_id: Some(42),
        };
        let message = types::MessageService {
            out: true,
            mentioned: false,
            media_unread: false,
            reactions_are_possible: false,
            silent: false,
            post: false,
            legacy: false,
            id: 77,
            from_id: None,
            peer_id: get_peer(USER),
            saved_peer_id: None,
            reply_to: None,
            date: 1,
            action: base::MessageAction::MessageActionTopicCreate(Box::new(action)),
            reactions: None,
            ttl_period: None,
        };
        let message = base::Message::MessageService(Box::new(message));
        let update = types::UpdateNewMessage { message, pts: 1, pts_count: 1 };
        let updates = vec![base::Update::UpdateNewMessage(Box::new(update))];
        let updates = types::Updates { updates, users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let (client, mut rx) = topic_client(vec![base::Updates::Updates(Box::new(updates)).to_bytes()]);

        let topic = client.create_forum_topic(USER, "Ideas").icon_color(0x6FB9F0).icon_emoji_id(42).await.unwrap();
        let expected = ForumTopic {
            id: 77,
            title: "Ideas".into(),
            icon_color: 0x6FB9F0,
            icon_emoji_id: Some(42),
            is_name_implicit: false,
            date: 1,
            top_message_id: 77,
            unread_messages_count: 0,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            is_closed: false,
            is_pinned: false,
            is_hidden: false,
        };
        assert_eq!(topic, expected);
        let request = messages::CreateForumTopic {
            title_missing: false,
            peer: base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 })),
            title: "Ideas".into(),
            icon_color: Some(0x6FB9F0),
            icon_emoji_id: Some(42),
            random_id: 0,
            send_as: None,
        };
        let (body, request) = (rx.try_recv().unwrap(), request.to_bytes());
        // random_id is random and written last, so only the bytes before it are compared.
        let fixed = request.len() - 8;
        assert_eq!((body.len(), &body[..fixed]), (request.len(), &request[..fixed]));
    }
}
