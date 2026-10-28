use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{ForumTopic, PeerRef};

pub struct CreateForumTopic<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    title: String,
    icon_color: Option<i32>,
    icon_emoji_id: Option<i64>,
}

impl Client {
    pub fn create_forum_topic(&self, chat_id: impl Into<PeerRef>, title: impl Into<String>) -> CreateForumTopic<'_> {
        CreateForumTopic {
            client: self,
            chat_id: chat_id.into(),
            title: title.into(),
            icon_color: None,
            icon_emoji_id: None,
        }
    }
}

impl CreateForumTopic<'_> {
    /// One of 0x6FB9F0, 0xFFD67E, 0xCB86DB, 0x8EEE98, 0xFF93B2 or 0xFB6F5F.
    pub fn icon_color(mut self, color: i32) -> Self {
        self.icon_color = Some(color);
        self
    }

    /// A custom emoji from [`Client::get_forum_topic_icon_stickers`].
    pub fn icon_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.icon_emoji_id = Some(custom_emoji_id);
        self
    }

    async fn send(self) -> Result<ForumTopic> {
        let client = self.client;
        let request = functions::messages::CreateForumTopic {
            title_missing: false,
            peer: client.resolve_peer(self.chat_id).await?,
            title: self.title,
            icon_color: self.icon_color,
            icon_emoji_id: self.icon_emoji_id,
            random_id: i64::from_le_bytes(random()),
            send_as: None,
        };
        let updates = match client.invoke(&request).await? {
            base::Updates::Updates(u) => u.updates,
            base::Updates::UpdatesCombined(u) => u.updates,
            base::Updates::UpdateShort(u) => vec![u.update],
            _ => Vec::new(),
        };
        updates
            .into_iter()
            .find_map(created_topic)
            .ok_or(Error::Unexpected("messages.createForumTopic returned no topic"))
    }
}

into_future!(CreateForumTopic, ForumTopic);

// A topic is identified by the id of the service message that created it.
fn created_topic(update: base::Update) -> Option<ForumTopic> {
    let message = match update {
        base::Update::UpdateNewMessage(u) => u.message,
        base::Update::UpdateNewChannelMessage(u) => u.message,
        _ => return None,
    };
    let base::Message::MessageService(m) = message else { return None };
    let (id, action) = (m.id, m.action);
    let base::MessageAction::MessageActionTopicCreate(a) = action else { return None };
    Some(ForumTopic {
        id,
        title: a.title,
        icon_color: a.icon_color,
        icon_emoji_id: a.icon_emoji_id,
        is_name_implicit: a.title_missing,
    })
}
