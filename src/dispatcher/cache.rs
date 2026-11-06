use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use super::Entities;
use crate::raw::base;
use crate::types::ForumTopic;
use crate::utils::{CHANNEL_OFFSET, channel_id_of};

/// At most `capacity` entries; inserting past it drops the one used longest ago, and `get` counts as a use.
pub(crate) struct Lru<K, V> {
    entries: HashMap<K, (V, u64)>,
    order: BTreeMap<u64, K>,
    tick: u64,
    capacity: usize,
}

impl<K: Hash + Eq + Clone, V> Lru<K, V> {
    fn new(capacity: usize) -> Self {
        Lru { entries: HashMap::new(), order: BTreeMap::new(), tick: 0, capacity }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        self.tick += 1;
        let (value, used) = self.entries.get_mut(key)?;
        self.order.remove(used);
        *used = self.tick;
        self.order.insert(self.tick, key.clone());
        Some(value)
    }

    fn insert(&mut self, key: K, value: V) {
        self.tick += 1;
        if let Some((_, used)) = self.entries.insert(key.clone(), (value, self.tick)) {
            self.order.remove(&used);
        }
        self.order.insert(self.tick, key);
        while self.entries.len() > self.capacity {
            let Some((_, oldest)) = self.order.pop_first() else { break };
            self.entries.remove(&oldest);
        }
    }

    fn remove(&mut self, key: &K) {
        if let Some((_, used)) = self.entries.remove(key) {
            self.order.remove(&used);
        }
    }

    fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        let order = &mut self.order;
        self.entries.retain(|key, (value, used)| {
            let kept = keep(key, value);
            if !kept {
                order.remove(used);
            }
            kept
        });
    }
}

/// A message as it came, with the users and chats of its batch, parsed again on every hit. A parsed `Message` holds
/// the `Client`, so keeping one here would keep the client alive forever.
#[derive(Clone)]
pub(crate) struct Cached {
    pub(crate) raw: base::Message,
    pub(crate) entities: Arc<Entities>,
}

// Outside channels message ids are per account, so 0 stands for every chat that isn't a channel.
type MessageKey = (i64, i32);

/// The message, forum topic and sticker set name caches, each off (`None`) unless its size is set.
#[derive(Default)]
pub(crate) struct Caches {
    messages: Option<Mutex<Lru<MessageKey, Cached>>>,
    topics: Option<Mutex<Lru<(i64, i32), ForumTopic>>>,
    set_names: Option<Mutex<Lru<i64, String>>>,
}

impl Caches {
    pub(crate) fn new(messages: usize, topics: usize) -> Caches {
        Caches {
            messages: (messages > 0).then(|| Mutex::new(Lru::new(messages))),
            topics: (topics > 0).then(|| Mutex::new(Lru::new(topics))),
            set_names: None,
        }
    }

    pub(crate) fn with_set_names(mut self, size: usize) -> Caches {
        self.set_names = (size > 0).then(|| Mutex::new(Lru::new(size)));
        self
    }

    /// The short name of the sticker set with this id, if looked up before.
    pub(crate) fn set_name(&self, set_id: i64) -> Option<String> {
        self.set_names.as_ref()?.lock().unwrap().get(&set_id).cloned()
    }

    pub(crate) fn store_set_name(&self, set_id: i64, name: &str) {
        if let Some(names) = &self.set_names {
            names.lock().unwrap().insert(set_id, name.to_owned());
        }
    }

    pub(crate) fn caches_messages(&self) -> bool {
        self.messages.is_some()
    }

    /// Learns from an update before it's routed: new and edited messages are kept, deleted ones dropped, and an
    /// edited topic forgotten. Reactions, views, forwards, pins and widget data change a message without an edit, so
    /// they drop it too; a later read asks Telegram. Other flag-only changes (a voice note marked listened, say)
    /// aren't tracked, so a cached message can show them stale.
    pub(crate) fn observe(&self, update: &base::Update, entities: &Arc<Entities>) {
        use base::Update as U;
        match update {
            U::UpdateNewMessage(u) => self.store(&u.message, entities),
            U::UpdateEditMessage(u) => self.store(&u.message, entities),
            U::UpdateNewChannelMessage(u) => self.store(&u.message, entities),
            U::UpdateEditChannelMessage(u) => self.store(&u.message, entities),
            U::UpdateDeleteMessages(u) => self.evict_ids(0, &u.messages),
            U::UpdateDeleteChannelMessages(u) => self.evict_ids(u.channel_id, &u.messages),
            U::UpdateMessageReactions(u) => self.evict_ids(channel_of(&u.peer), &[u.msg_id]),
            U::UpdateChannelMessageViews(u) => self.evict_ids(u.channel_id, &[u.id]),
            U::UpdateChannelMessageForwards(u) => self.evict_ids(u.channel_id, &[u.id]),
            U::UpdatePinnedMessages(u) => self.evict_ids(channel_of(&u.peer), &u.messages),
            U::UpdatePinnedChannelMessages(u) => self.evict_ids(u.channel_id, &u.messages),
            U::UpdateMessageWidgetData(u) => self.evict_ids(channel_of(&u.peer), &[u.msg_id]),
            U::UpdateEditForumTopic(u) => self.evict_topic(crate::utils::get_peer_id(&u.peer), u.topic_id),
            _ => {}
        }
    }

    /// Keeps a regular message, sent or received; scheduled and business messages never come here. Polls are left
    /// out: their votes change without an edit.
    pub(crate) fn store(&self, raw: &base::Message, entities: &Arc<Entities>) {
        let topic_edit = match raw {
            base::Message::MessageService(m) if matches!(m.action, base::MessageAction::MessageActionTopicEdit(_)) => {
                Some(m)
            }
            _ => None,
        };
        if let Some(m) = topic_edit {
            self.forget_topic(&m.peer_id, m.reply_to.as_ref());
        }
        let Some(messages) = &self.messages else { return };
        let (peer, id) = match raw {
            base::Message::Message(m) if matches!(m.media, Some(base::MessageMedia::MessageMediaPoll(_))) => return,
            base::Message::Message(m) => (&m.peer_id, m.id),
            base::Message::MessageService(m) => (&m.peer_id, m.id),
            base::Message::MessageEmpty(_) => return,
        };
        let cached = Cached { raw: raw.clone(), entities: entities.clone() };
        messages.lock().unwrap().insert((channel_of(peer), id), cached);
    }

    fn evict_ids(&self, channel: i64, ids: &[i32]) {
        let Some(messages) = &self.messages else { return };
        let mut messages = messages.lock().unwrap();
        ids.iter().for_each(|&id| messages.remove(&(channel, id)));
    }

    /// Drops messages this account deleted itself: Telegram answers those with a pts count, not a delete update.
    pub(crate) fn evict(&self, chat_id: i64, ids: &[i32]) {
        self.evict_ids(channel_of_chat(chat_id), ids);
    }

    /// Drops every cached message of a chat, after its history (or a topic's) was deleted.
    pub(crate) fn evict_chat(&self, chat_id: i64) {
        let Some(messages) = &self.messages else { return };
        let peer = crate::utils::get_peer(chat_id);
        messages.lock().unwrap().retain(|_, cached| match &cached.raw {
            base::Message::Message(m) => m.peer_id != peer,
            base::Message::MessageService(m) => m.peer_id != peer,
            base::Message::MessageEmpty(_) => false,
        });
    }

    pub(crate) fn evict_topic(&self, chat_id: i64, topic_id: i32) {
        if let Some(topics) = &self.topics {
            topics.lock().unwrap().remove(&(chat_id, topic_id));
        }
    }

    /// The cached message `id` of the chat with this Bot-API id.
    pub(crate) fn message(&self, chat_id: i64, id: i32) -> Option<Cached> {
        self.messages.as_ref()?.lock().unwrap().get(&(channel_of_chat(chat_id), id)).cloned()
    }

    pub(crate) fn topic(&self, chat_id: i64, topic_id: i32) -> Option<ForumTopic> {
        self.topics.as_ref()?.lock().unwrap().get(&(chat_id, topic_id)).cloned()
    }

    pub(crate) fn store_topic(&self, chat_id: i64, topic: &ForumTopic) {
        if let Some(topics) = &self.topics {
            topics.lock().unwrap().insert((chat_id, topic.id), topic.clone());
        }
    }

    fn forget_topic(&self, peer: &base::Peer, header: Option<&base::MessageReplyHeader>) {
        let Some(topics) = &self.topics else { return };
        // The thread is found the way filters find it; a topic edit outside any thread is the General topic's.
        let thread = match header {
            Some(base::MessageReplyHeader::MessageReplyHeader(h)) if h.forum_topic => {
                h.reply_to_top_id.or(h.reply_to_msg_id).unwrap_or(GENERAL_TOPIC)
            }
            _ => GENERAL_TOPIC,
        };
        topics.lock().unwrap().remove(&(crate::utils::get_peer_id(peer), thread));
    }
}

const GENERAL_TOPIC: i32 = 1;

fn channel_of(peer: &base::Peer) -> i64 {
    match peer {
        base::Peer::PeerChannel(c) => c.channel_id,
        _ => 0,
    }
}

fn channel_of_chat(chat_id: i64) -> i64 {
    if chat_id < -CHANNEL_OFFSET { channel_id_of(chat_id) } else { 0 }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::mpsc::UnboundedReceiver;

    use super::{Caches, Lru};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::messages_and_media::message::tests::{header, service};
    use crate::types::{ForumTopic, Message};
    use crate::utils::{get_channel_id, get_peer};

    fn raw(id: i32, chat_id: i64) -> base::Message {
        base::Message::Message(Box::new(Message::bare(id, get_peer(chat_id), 1)))
    }

    fn cached_ids(caches: &Caches, chat_id: i64, ids: &[i32]) -> Vec<bool> {
        ids.iter().map(|&id| caches.message(chat_id, id).is_some()).collect()
    }

    #[test]
    fn lru_evicts_the_least_recently_used() {
        let mut lru = Lru::new(2);
        let get = |lru: &mut Lru<i32, &'static str>, key| lru.get(&key).copied();
        lru.insert(1, "a");
        lru.insert(2, "b");
        assert_eq!(get(&mut lru, 1), Some("a"));
        lru.insert(3, "c");
        assert_eq!([get(&mut lru, 1), get(&mut lru, 2), get(&mut lru, 3)], [Some("a"), None, Some("c")]);
        lru.insert(1, "A");
        lru.remove(&3);
        assert_eq!([get(&mut lru, 1), get(&mut lru, 3)], [Some("A"), None]);
        assert_eq!((lru.entries.len(), lru.order.len()), (1, 1));
    }

    #[test]
    fn size_zero_keeps_nothing() {
        let caches = Caches::new(0, 0);
        assert!(caches.messages.is_none() && caches.topics.is_none() && !caches.caches_messages());
        caches.store(&raw(5, 7), &Arc::default());
        assert!(caches.message(7, 5).is_none());
    }

    #[test]
    fn updates_are_cached_under_channel_and_id() {
        let caches = Caches::new(10, 0);
        let entities = Arc::new(Entities::default());
        let new = |message| {
            base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 1, pts_count: 1 }))
        };
        let channel = |message| {
            base::Update::UpdateNewChannelMessage(Box::new(types::UpdateNewChannelMessage {
                message,
                pts: 1,
                pts_count: 1,
            }))
        };
        caches.observe(&new(raw(5, 7)), &entities);
        caches.observe(&channel(raw(5, get_channel_id(9))), &entities);
        // A private chat's and a group's ids are the account's: any of its chats finds message 5.
        assert_eq!(cached_ids(&caches, 7, &[5, 6]), [true, false]);
        assert_eq!(cached_ids(&caches, -42, &[5]), [true]);
        assert_eq!(cached_ids(&caches, get_channel_id(9), &[5]), [true]);
        assert_eq!(cached_ids(&caches, get_channel_id(8), &[5]), [false]);
        assert!(Arc::ptr_eq(&caches.message(7, 5).unwrap().entities, &entities));
    }

    #[test]
    fn edits_replace_and_deletes_evict() {
        let caches = Caches::new(10, 0);
        let mut edited = Message::bare(5, get_peer(7), 1);
        edited.message = "edited".into();
        caches.store(&raw(5, 7), &Arc::default());
        let edit = types::UpdateEditMessage { message: base::Message::Message(Box::new(edited)), pts: 1, pts_count: 1 };
        caches.observe(&base::Update::UpdateEditMessage(Box::new(edit)), &Arc::default());
        let base::Message::Message(m) = caches.message(7, 5).unwrap().raw else { panic!("not a message") };
        assert_eq!(m.message, "edited");

        caches.store(&raw(6, get_channel_id(9)), &Arc::default());
        let deleted = types::UpdateDeleteMessages { messages: vec![5], pts: 1, pts_count: 1 };
        caches.observe(&base::Update::UpdateDeleteMessages(Box::new(deleted)), &Arc::default());
        let deleted = types::UpdateDeleteChannelMessages { channel_id: 9, messages: vec![6], pts: 1, pts_count: 1 };
        caches.observe(&base::Update::UpdateDeleteChannelMessages(Box::new(deleted)), &Arc::default());
        assert_eq!(
            (cached_ids(&caches, 7, &[5]), cached_ids(&caches, get_channel_id(9), &[6])),
            (vec![false], vec![false])
        );
    }

    #[test]
    fn polls_and_empty_messages_are_skipped() {
        let caches = Caches::new(10, 0);
        let question = types::TextWithEntities { text: "?".into(), entities: Vec::new() };
        let poll = types::Poll {
            id: 1,
            closed: false,
            public_voters: false,
            multiple_choice: false,
            quiz: false,
            open_answers: false,
            revoting_disabled: false,
            shuffle_answers: false,
            hide_results_until_close: false,
            creator: false,
            subscribers_only: false,
            question: base::TextWithEntities::TextWithEntities(Box::new(question)),
            answers: Vec::new(),
            close_period: None,
            close_date: None,
            countries_iso2: None,
            hash: 0,
        };
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
            poll: base::Poll::Poll(Box::new(poll)),
            results: base::PollResults::PollResults(Box::new(results)),
            attached_media: None,
        };
        let mut with_poll = Message::bare(5, get_peer(7), 1);
        with_poll.media = Some(base::MessageMedia::MessageMediaPoll(Box::new(media)));
        caches.store(&base::Message::Message(Box::new(with_poll)), &Arc::default());
        let empty = base::Message::MessageEmpty(Box::new(types::MessageEmpty { id: 6, peer_id: None }));
        caches.store(&empty, &Arc::default());
        assert_eq!(cached_ids(&caches, 7, &[5, 6]), [false, false]);
    }

    fn topic(id: i32) -> ForumTopic {
        ForumTopic {
            id,
            title: "t".into(),
            icon_color: 0,
            icon_emoji_id: None,
            is_name_implicit: false,
            date: 0,
            top_message_id: 0,
            unread_messages_count: 0,
            unread_mentions_count: 0,
            unread_reactions_count: 0,
            is_closed: false,
            is_pinned: false,
            is_hidden: false,
            creator: None,
            is_my: false,
            is_short: false,
            unread_poll_vote_count: 0,
        }
    }

    /// An edit of topic `thread` in `forum`; `None` is the General topic, which has no thread.
    fn topic_edit(forum: i64, thread: Option<i32>) -> base::Message {
        let edit = types::MessageActionTopicEdit {
            title: Some("new".into()),
            icon_emoji_id: None,
            closed: None,
            hidden: None,
        };
        let reply_to = thread.map(|id| {
            let in_topic = types::MessageReplyHeader { forum_topic: true, reply_to_msg_id: Some(id), ..header() };
            base::MessageReplyHeader::MessageReplyHeader(Box::new(in_topic))
        });
        let service = service(base::MessageAction::MessageActionTopicEdit(Box::new(edit)));
        base::Message::MessageService(Box::new(types::MessageService { peer_id: get_peer(forum), reply_to, ..service }))
    }

    #[test]
    fn a_topic_edit_evicts_its_topic() {
        let caches = Caches::new(0, 10);
        let forum = get_channel_id(9);
        caches.store_topic(forum, &topic(6));
        caches.store_topic(forum, &topic(1));
        assert_eq!(caches.topic(forum, 6).map(|t| t.id), Some(6));
        caches.store(&topic_edit(forum, Some(6)), &Arc::default());
        assert_eq!((caches.topic(forum, 6), caches.topic(forum, 1).map(|t| t.id)), (None, Some(1)));
        caches.store(&topic_edit(forum, None), &Arc::default());
        assert_eq!(caches.topic(forum, 1), None);
    }

    #[test]
    fn a_forum_topic_edit_update_evicts_its_topic() {
        let caches = Caches::new(0, 10);
        let forum = get_channel_id(9);
        caches.store_topic(forum, &topic(6));
        caches.store_topic(forum, &topic(7));
        let edit = types::UpdateEditForumTopic {
            peer: get_peer(forum),
            topic_id: 6,
            title: Some("new".into()),
            icon_emoji_id: None,
            closed: None,
            hidden: None,
        };
        caches.observe(&base::Update::UpdateEditForumTopic(Box::new(edit)), &Arc::default());
        assert_eq!((caches.topic(forum, 6), caches.topic(forum, 7).map(|t| t.id)), (None, Some(7)));
    }

    fn caching(messages: usize, topics: usize) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (mut client, rx) = Client::recording();
        Arc::get_mut(&mut client.0).unwrap().caches = Caches::new(messages, topics);
        (client, rx)
    }

    fn message(client: &Client, id: i32, chat_id: i64) -> Message {
        Message::parse(client, Box::new(Message::bare(id, get_peer(chat_id), 1)), &Entities::default())
    }

    #[tokio::test]
    async fn sent_messages_are_cached() {
        let (client, _rx) = caching(10, 0);
        let short = types::UpdateShortSentMessage {
            out: true,
            id: 5,
            pts: 1,
            pts_count: 1,
            date: 1,
            media: None,
            entities: None,
            ttl_period: None,
        };
        let short = base::Updates::UpdateShortSentMessage(Box::new(short));
        Message::from_updates(&client, short, 7, ("hi".into(), None)).unwrap();
        let new = types::UpdateNewMessage { message: raw(6, get_channel_id(9)), pts: 1, pts_count: 1 };
        let scheduled = types::UpdateNewScheduledMessage { message: raw(7, 7) };
        let updates = types::Updates {
            updates: vec![
                base::Update::UpdateNewMessage(Box::new(new)),
                base::Update::UpdateNewScheduledMessage(Box::new(scheduled)),
            ],
            users: Vec::new(),
            chats: Vec::new(),
            date: 1,
            seq: 0,
        };
        assert_eq!(Message::all_from_updates(&client, base::Updates::Updates(Box::new(updates))).count(), 2);
        let caches = &client.0.caches;
        assert_eq!(cached_ids(caches, 7, &[5, 7]), [true, false]);
        assert_eq!(cached_ids(caches, get_channel_id(9), &[6]), [true]);
    }

    #[tokio::test]
    async fn reply_to_message_hits_the_cache_without_a_request() {
        let (client, mut rx) = caching(10, 0);
        client.0.caches.store(&raw(4, 7), &Arc::default());
        let mut reply = message(&client, 5, 7);
        reply.reply_to_message_id = Some(4);
        assert_eq!(reply.reply_to_message().await.unwrap().map(|m| (m.id, m.chat.id)), Some((4, 7)));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn callback_message_hits_the_cache() {
        let (client, mut rx) = caching(10, 0);
        client.0.caches.store(&raw(5, 7), &Arc::default());
        let press = crate::dispatcher::listeners::tests::query(&client, 7, 7, 5);
        assert_eq!(press.message().await.unwrap().map(|m| m.id), Some(5));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn topic_hits_the_cache_and_an_edit_evicts_it() {
        let (client, mut rx) = caching(0, 10);
        let forum = get_channel_id(9);
        let input =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(forum, input.clone());
        client.0.caches.store_topic(forum, &topic(6));
        let mut in_topic = message(&client, 1, forum);
        (in_topic.chat.is_forum, in_topic.message_thread_id) = (true, Some(6));
        assert_eq!(in_topic.topic().await.unwrap().map(|t| t.id), Some(6));
        assert!(rx.try_recv().is_err());

        let edit = types::UpdateNewChannelMessage { message: topic_edit(forum, Some(6)), pts: 1, pts_count: 1 };
        client.0.caches.observe(&base::Update::UpdateNewChannelMessage(Box::new(edit)), &Arc::default());
        let _ = in_topic.topic().await;
        let fetch = functions::messages::GetForumTopicsByID { peer: input, topics: vec![6] };
        assert_eq!(rx.try_recv().unwrap(), fetch.to_bytes());
    }

    #[tokio::test]
    async fn a_default_client_still_fetches() {
        let (client, mut rx) = Client::recording();
        client.0.caches.store(&raw(4, 7), &Arc::default());
        client.0.caches.store_topic(get_channel_id(9), &topic(6));
        let mut reply = message(&client, 5, 7);
        reply.reply_to_message_id = Some(4);
        let _ = reply.reply_to_message().await;
        let id = base::InputMessage::InputMessageID(types::InputMessageID { id: 4 });
        assert_eq!(rx.try_recv().unwrap(), functions::messages::GetMessages { id: vec![id] }.to_bytes());
        let press = crate::dispatcher::listeners::tests::query(&client, 7, 7, 4);
        let _ = press.message().await;
        assert!(rx.try_recv().is_ok());
    }

    fn get_messages(id: i32) -> Vec<u8> {
        let id = base::InputMessage::InputMessageID(types::InputMessageID { id });
        functions::messages::GetMessages { id: vec![id] }.to_bytes()
    }

    #[tokio::test]
    async fn own_deletes_evict() {
        let affected = types::messages::AffectedMessages { pts: 1, pts_count: 1 };
        let affected = Ok(base::messages::AffectedMessages::AffectedMessages(affected).to_bytes());
        let (mut client, mut rx) = Client::scripted(vec![affected]);
        Arc::get_mut(&mut client.0).unwrap().caches = Caches::new(10, 0);
        client.0.caches.store(&raw(4, 7), &Arc::default());
        client.delete_messages(7, &[4]).await.unwrap();
        rx.try_recv().unwrap();
        // Gone from the cache, so the press asks Telegram again.
        let press = crate::dispatcher::listeners::tests::query(&client, 7, 7, 4);
        let _ = press.message().await;
        assert_eq!(rx.try_recv().unwrap(), get_messages(4));
    }

    #[tokio::test]
    async fn own_reactions_evict() {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        let (mut client, _rx) = Client::scripted(vec![Ok(base::Updates::Updates(Box::new(updates)).to_bytes())]);
        Arc::get_mut(&mut client.0).unwrap().caches = Caches::new(10, 0);
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, user);
        client.0.caches.store(&raw(4, 7), &Arc::default());
        client.send_reaction(7, 4).reaction("👍").await.unwrap();
        assert!(client.0.caches.message(7, 4).is_none());
    }

    #[test]
    fn reactions_views_widget_data_pins_and_history_deletes_evict() {
        let caches = Caches::new(10, 0);
        let channel = get_channel_id(9);
        for (id, chat) in [(1, 7), (2, 7), (3, channel), (4, channel), (5, 8), (6, channel)] {
            caches.store(&raw(id, chat), &Arc::default());
        }
        let reactions = types::UpdateMessageReactions {
            peer: get_peer(7),
            msg_id: 1,
            top_msg_id: None,
            saved_peer_id: None,
            reactions: base::MessageReactions::MessageReactions(Box::new(types::MessageReactions {
                min: false,
                can_see_list: false,
                reactions_as_tags: false,
                results: Vec::new(),
                recent_reactions: None,
                top_reactors: None,
            })),
        };
        caches.observe(&base::Update::UpdateMessageReactions(Box::new(reactions)), &Arc::default());
        let views = types::UpdateChannelMessageViews { channel_id: 9, id: 3, views: 10 };
        caches.observe(&base::Update::UpdateChannelMessageViews(Box::new(views)), &Arc::default());
        let widget = types::UpdateMessageWidgetData {
            peer: get_peer(channel),
            msg_id: 6,
            public_data: Some("{}".into()),
            public_data_hash: Some(1),
            chat_data: None,
            pts: 1,
            pts_count: 1,
        };
        caches.observe(&base::Update::UpdateMessageWidgetData(Box::new(widget)), &Arc::default());
        assert_eq!(cached_ids(&caches, 7, &[1, 2, 5]), [false, true, true]);
        assert_eq!(cached_ids(&caches, channel, &[3, 4, 6]), [false, true, false]);
        // A history delete drops only that chat's messages, though ids outside channels share one key space.
        caches.evict_chat(7);
        caches.evict_chat(channel);
        assert_eq!(
            (cached_ids(&caches, 7, &[2, 5]), cached_ids(&caches, channel, &[4])),
            (vec![false, true], vec![false])
        );
    }
}
