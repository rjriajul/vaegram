use crate::Client;
use crate::crypto::random;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Chat, ForumTopic, PeerRef};

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
        let (updates, entities) = match client.invoke(&request).await? {
            base::Updates::Updates(u) => (u.updates, Entities::new(client, u.users, u.chats)),
            base::Updates::UpdatesCombined(u) => (u.updates, Entities::new(client, u.users, u.chats)),
            base::Updates::UpdateShort(u) => (vec![u.update], Entities::default()),
            _ => (Vec::new(), Entities::default()),
        };
        updates
            .into_iter()
            .find_map(|update| created_topic(update, &entities))
            .ok_or(Error::Unexpected("messages.createForumTopic returned no topic"))
    }
}

into_future!(CreateForumTopic, ForumTopic);

// A topic is identified by the id of the service message that created it.
fn created_topic(update: base::Update, entities: &Entities) -> Option<ForumTopic> {
    let message = match update {
        base::Update::UpdateNewMessage(u) => u.message,
        base::Update::UpdateNewChannelMessage(u) => u.message,
        _ => return None,
    };
    let base::Message::MessageService(m) = message else { return None };
    let (id, date, action, from_id) = (m.id, m.date, m.action, m.from_id);
    let base::MessageAction::MessageActionTopicCreate(a) = action else { return None };
    Some(ForumTopic {
        id,
        title: a.title,
        icon_color: a.icon_color,
        icon_emoji_id: a.icon_emoji_id,
        is_name_implicit: a.title_missing,
        date,
        top_message_id: id,
        unread_messages_count: 0,
        unread_mentions_count: 0,
        unread_reactions_count: 0,
        is_closed: false,
        is_pinned: false,
        is_hidden: false,
        creator: from_id.map(|peer| Chat::parse(&peer, entities, false)),
        is_my: true,
        is_short: false,
        unread_poll_vote_count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::created_topic;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::Chat;
    use crate::utils::{get_channel_id, get_peer};

    #[test]
    fn created_topic_is_mine_and_names_its_creator() {
        let action = types::MessageActionTopicCreate {
            title_missing: true,
            title: "Ideas".into(),
            icon_color: 0x6FB9F0,
            icon_emoji_id: None,
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
            from_id: Some(get_peer(7)),
            peer_id: get_peer(get_channel_id(9)),
            saved_peer_id: None,
            reply_to: None,
            date: 1,
            action: base::MessageAction::MessageActionTopicCreate(Box::new(action)),
            reactions: None,
            ttl_period: None,
        };
        let message = base::Message::MessageService(Box::new(message));
        let update = types::UpdateNewChannelMessage { message, pts: 1, pts_count: 1 };
        let entities = Entities::default();
        let topic = created_topic(base::Update::UpdateNewChannelMessage(Box::new(update)), &entities).unwrap();
        assert_eq!((topic.id, topic.top_message_id, topic.is_name_implicit), (77, 77, true));
        assert_eq!((topic.is_my, topic.is_short, topic.unread_poll_vote_count), (true, false, 0));
        assert_eq!(topic.creator, Some(Chat::parse(&get_peer(7), &entities, false)));

        let other = base::Update::UpdateNewChannelMessage(Box::new(types::UpdateNewChannelMessage {
            message: base::Message::MessageEmpty(Box::new(types::MessageEmpty { id: 1, peer_id: None })),
            pts: 1,
            pts_count: 1,
        }));
        assert_eq!(created_topic(other, &entities), None);
    }
}
