use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use tokio::time::Instant;

use crate::Client;
use crate::client::Fetch;
use crate::methods::advanced::without_retries;
use crate::raw::{base, functions, types};
use crate::types::{ForumTopic, Media, Message, Story};

// What each wanted `(chat, id)` turned out to be: `None` when Telegram has nothing there. A failed request leaves
// its keys out, so the helpers ask again on demand.
type Found<T> = HashMap<(i64, i32), Option<T>>;

// Sticker set lookups in flight at once.
const SET_LOOKUPS: usize = 4;

/// `future`'s output if it comes by `deadline`; no deadline (a `request_timeout` too long to add) waits for it.
async fn within<F: Future>(deadline: Option<Instant>, future: F) -> Option<F::Output> {
    match deadline {
        Some(deadline) => tokio::time::timeout_at(deadline, future).await.ok(),
        None => Some(future.await),
    }
}

impl Client {
    /// Fills what the fetch options ask for, for messages handlers are about to see or a read is about to return:
    /// the replied and pinned messages, forum topics, stories and sticker set names, in one batch for all of them.
    /// A fetched message gets its topic, stories and set names too, but no reply or pin of its own. The caller
    /// waits for this, so nothing here sleeps off a FLOOD_WAIT or waits past `request_timeout`. Each kind stands
    /// alone: one that floods, fails or runs out of time leaves only its own slots empty, and the helpers ask on
    /// demand.
    pub(crate) async fn prefetch(&self, messages: &mut [Message]) {
        if !self.0.fetch.any() || messages.is_empty() {
            return;
        }
        let deadline = Instant::now().checked_add(self.0.options.request_timeout);
        without_retries(self.fill(messages, deadline)).await;
    }

    /// `prefetch` for a read whose answer has holes where messages are missing.
    pub(crate) async fn prefetch_found(&self, messages: &mut [Option<Message>]) {
        if !self.0.fetch.any() {
            return;
        }
        let at: Vec<usize> = messages.iter().enumerate().filter(|(_, m)| m.is_some()).map(|(i, _)| i).collect();
        let mut found: Vec<Message> = messages.iter_mut().filter_map(Option::take).collect();
        self.prefetch(&mut found).await;
        for (i, message) in at.into_iter().zip(found) {
            messages[i] = Some(message);
        }
    }

    // A kind's slots change only once all of it has come, so a timeout can't leave that kind half filled.
    async fn fill(&self, messages: &mut [Message], deadline: Option<Instant>) {
        let fetch = self.0.fetch;
        let mut found = match fetch.replies {
            true => within(deadline, self.fetch_messages(messages)).await.unwrap_or_default(),
            false => Found::new(),
        };
        let bot = self.0.me.lock().unwrap().is_bot;
        let wanted = Wanted::of(fetch, bot, messages.iter().chain(found.values().flatten()));
        let (topics, stories, names) = tokio::join!(
            within(deadline, self.fetch_topics(wanted.topics)),
            within(deadline, self.fetch_stories(wanted.stories)),
            within(deadline, self.fetch_set_names(wanted.sets))
        );
        let extras = Extras {
            topics: topics.unwrap_or_default(),
            stories: stories.unwrap_or_default(),
            names: names.unwrap_or_default(),
        };
        found.values_mut().flatten().for_each(|m| extras.apply(m));
        for message in messages {
            extras.apply(message);
            let chat = message.chat.id;
            let reply = message.reply_id().and_then(|id| found.get(&(chat, id))).cloned();
            let pinned = message.pinned_id().and_then(|id| found.get(&(chat, id))).cloned();
            if reply.is_some() {
                message.prefetched.get_or_insert_with(Default::default).reply = reply;
            }
            if pinned.is_some() {
                message.prefetched.get_or_insert_with(Default::default).pinned = pinned;
            }
        }
    }

    // One request per chat for what the message cache doesn't have; what comes back goes into the cache.
    async fn fetch_messages(&self, messages: &[Message]) -> Found<Message> {
        let mut wanted = BTreeMap::new();
        for message in messages {
            for id in message.reply_id().into_iter().chain(message.pinned_id()) {
                push(&mut wanted, message.chat.id, id);
            }
        }
        let mut found = Found::new();
        for (chat, ids) in wanted {
            let mut missing = Vec::new();
            for id in ids {
                match self.0.caches.message(chat, id) {
                    Some(c) => {
                        found.insert((chat, id), Message::from_raw(self, c.raw, &c.entities));
                    }
                    None => missing.push(id),
                }
            }
            if missing.is_empty() {
                continue;
            }
            let Ok((raws, entities)) = self.get_raw_messages(chat, &missing).await else { continue };
            let entities = Arc::new(entities);
            for (id, raw) in missing.into_iter().zip(raws) {
                let message = raw.and_then(|raw| {
                    let raw = base::Message::Message(raw);
                    self.0.caches.store(&raw, &entities);
                    Message::from_raw(self, raw, &entities)
                });
                found.insert((chat, id), message);
            }
        }
        found
    }

    async fn fetch_topics(&self, wanted: BTreeMap<i64, Vec<i32>>) -> Found<ForumTopic> {
        let mut found = Found::new();
        for (chat, ids) in wanted {
            let mut missing = Vec::new();
            for id in ids {
                match self.0.caches.topic(chat, id) {
                    Some(topic) => {
                        found.insert((chat, id), Some(topic));
                    }
                    None => missing.push(id),
                }
            }
            if missing.is_empty() {
                continue;
            }
            let Ok(topics) = self.get_forum_topics_by_id(chat, &missing).await else { continue };
            for id in missing {
                let topic = topics.iter().find(|t| t.id == id).cloned();
                if let Some(topic) = &topic {
                    self.0.caches.store_topic(chat, topic);
                }
                found.insert((chat, id), topic);
            }
        }
        found
    }

    async fn fetch_stories(&self, wanted: BTreeMap<i64, Vec<i32>>) -> Found<Story> {
        let mut found = Found::new();
        for (chat, ids) in wanted {
            let Ok(stories) = self.get_stories(chat, &ids).await else { continue };
            for id in ids {
                found.insert((chat, id), stories.iter().find(|s| s.id == id).cloned());
            }
        }
        found
    }

    // One messages.getStickerSet per set the name cache doesn't know, SET_LOOKUPS at a time; a set that fails stays
    // unnamed.
    async fn fetch_set_names(&self, wanted: BTreeMap<i64, i64>) -> HashMap<i64, String> {
        let mut lanes: [Vec<(i64, i64)>; SET_LOOKUPS] = Default::default();
        for (i, set) in wanted.into_iter().enumerate() {
            lanes[i % SET_LOOKUPS].push(set);
        }
        let [a, b, c, d] = lanes;
        let (a, b, c, d) = tokio::join!(self.set_names(a), self.set_names(b), self.set_names(c), self.set_names(d));
        a.into_iter().chain(b).chain(c).chain(d).collect()
    }

    async fn set_names(&self, sets: Vec<(i64, i64)>) -> Vec<(i64, String)> {
        let mut names = Vec::new();
        for (id, access_hash) in sets {
            if let Some(name) = self.0.caches.set_name(id) {
                names.push((id, name));
                continue;
            }
            let stickerset =
                base::InputStickerSet::InputStickerSetID(Box::new(types::InputStickerSetID { id, access_hash }));
            let request = functions::messages::GetStickerSet { stickerset, hash: 0 };
            let Ok(base::messages::StickerSet::StickerSet(set)) = self.invoke(&request).await else { continue };
            let base::StickerSet::StickerSet(set) = &set.set;
            self.0.caches.store_set_name(id, &set.short_name);
            names.push((id, set.short_name.clone()));
        }
        names
    }
}

impl Message {
    // A business chat's message ids belong to the other account, so the bot can't look them up.
    fn reply_id(&self) -> Option<i32> {
        self.reply_to_message_id.filter(|_| self.business_connection_id.is_none())
    }
}

/// The topics and stories (by chat) and sticker sets (id to access hash) a batch of messages points at.
#[derive(Default)]
struct Wanted {
    topics: BTreeMap<i64, Vec<i32>>,
    stories: BTreeMap<i64, Vec<i32>>,
    sets: BTreeMap<i64, i64>,
}

impl Wanted {
    // Bots can fetch neither topics by id nor stories; a business message's ids belong to the other account, so it
    // gets only its sticker set names.
    fn of<'a>(fetch: Fetch, bot: bool, messages: impl Iterator<Item = &'a Message>) -> Wanted {
        let mut wanted = Wanted::default();
        for message in messages {
            let own = !bot && message.business_connection_id.is_none();
            if let Some(topic) = message.topic_id().filter(|_| fetch.topics && own) {
                push(&mut wanted.topics, message.chat.id, topic);
            }
            let stories = fetch.stories && own;
            match &message.media {
                Some(Media::Story(story)) if stories && is_partial(story) => {
                    push(&mut wanted.stories, story.chat.id, story.id);
                }
                Some(Media::Sticker(sticker)) if fetch.stickers && sticker.set_name.is_none() => {
                    if let Some((id, access_hash)) = sticker.set {
                        wanted.sets.insert(id, access_hash);
                    }
                }
                _ => {}
            }
            if let Some((id, chat)) = message.reply_to_story_id.zip(message.reply_to_story_user_id).filter(|_| stories)
            {
                push(&mut wanted.stories, chat, id);
            }
        }
        wanted
    }
}

/// Story media that came as a bare reference, or that Telegram skipped, carries only the chat and the id.
fn is_partial(story: &Story) -> bool {
    story.is_skipped || story.media.is_none()
}

fn push(wanted: &mut BTreeMap<i64, Vec<i32>>, chat: i64, id: i32) {
    let ids = wanted.entry(chat).or_default();
    if !ids.contains(&id) {
        ids.push(id);
    }
}

struct Extras {
    topics: Found<ForumTopic>,
    stories: Found<Story>,
    names: HashMap<i64, String>,
}

impl Extras {
    fn apply(&self, message: &mut Message) {
        let chat = message.chat.id;
        let topic = message.topic_id().and_then(|id| self.topics.get(&(chat, id))).cloned();
        let story = match (message.reply_to_story_id, message.reply_to_story_user_id) {
            (Some(id), Some(chat)) => self.stories.get(&(chat, id)).cloned(),
            _ => None,
        };
        if topic.is_some() {
            message.prefetched.get_or_insert_with(Default::default).topic = topic;
        }
        if story.is_some() {
            message.prefetched.get_or_insert_with(Default::default).story = story;
        }
        match &mut message.media {
            Some(Media::Story(story)) => {
                if let Some(Some(full)) = self.stories.get(&(story.chat.id, story.id)) {
                    **story = full.clone();
                }
            }
            Some(Media::Sticker(sticker)) => {
                if let Some(name) = sticker.set.and_then(|(id, _)| self.names.get(&id)) {
                    sticker.set_name = Some(name.clone());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;
    use tokio::time::Instant;

    use crate::Client;
    use crate::client::Fetch;
    use crate::dispatcher::Entities;
    use crate::dispatcher::cache::Caches;
    use crate::errors::{Error, Result, RpcError};
    use crate::handlers::Routed;
    use crate::raw::functions::{messages, stories};
    use crate::raw::{Serializable, base, types};
    use crate::types::messages_and_media::message::tests::header;
    use crate::types::{Media, Message, ServiceAction, Sticker, Story};
    use crate::utils::{get_channel_id, get_peer};

    const REPLIES: Fetch = Fetch { replies: true, topics: false, stories: false, stickers: false };

    fn client(fetch: Fetch, bot: bool, replies: Vec<Result<Vec<u8>>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (mut client, sent) = Client::scripted(replies);
        let inner = Arc::get_mut(&mut client.0).unwrap();
        (inner.fetch, inner.caches) = (fetch, Caches::new(10, 10).with_set_names(10));
        inner.me.get_mut().unwrap().is_bot = bot;
        (client, sent)
    }

    fn message(client: &Client, id: i32) -> Message {
        Message::parse(client, Box::new(Message::bare(id, get_peer(-5), 0)), &Entities::default())
    }

    fn found(messages: Vec<types::Message>) -> Result<Vec<u8>> {
        let messages = types::messages::Messages {
            messages: messages.into_iter().map(|m| base::Message::Message(Box::new(m))).collect(),
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        Ok(base::messages::Messages::Messages(Box::new(messages)).to_bytes())
    }

    fn get_messages(ids: &[i32]) -> Vec<u8> {
        let id = ids.iter().map(|&id| base::InputMessage::InputMessageID(types::InputMessageID { id })).collect();
        messages::GetMessages { id }.to_bytes()
    }

    fn refused(error: &str) -> Result<Vec<u8>> {
        Err(Error::Rpc(RpcError::new(400, error)))
    }

    fn set_id(id: i64) -> base::InputStickerSet {
        base::InputStickerSet::InputStickerSetID(Box::new(types::InputStickerSetID { id, access_hash: 11 }))
    }

    fn sticker(set: i64) -> Sticker {
        let attribute = types::DocumentAttributeSticker {
            mask: false,
            alt: "x".into(),
            stickerset: set_id(set),
            mask_coords: None,
        };
        let document = types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "image/webp".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: vec![base::DocumentAttribute::DocumentAttributeSticker(Box::new(attribute))],
        };
        Sticker::parse(&base::Document::Document(Box::new(document))).unwrap()
    }

    fn sticker_set(id: i64, short_name: &str) -> Result<Vec<u8>> {
        let set = types::StickerSet {
            archived: false,
            official: false,
            masks: false,
            emojis: false,
            text_color: false,
            channel_emoji_status: false,
            creator: false,
            installed_date: None,
            id,
            access_hash: 11,
            title: "Cats".into(),
            short_name: short_name.into(),
            thumbs: None,
            thumb_dc_id: None,
            thumb_version: None,
            thumb_document_id: None,
            count: 0,
            hash: 0,
        };
        let set = types::messages::StickerSet {
            set: base::StickerSet::StickerSet(Box::new(set)),
            packs: Vec::new(),
            keywords: Vec::new(),
            documents: Vec::new(),
        };
        Ok(base::messages::StickerSet::StickerSet(Box::new(set)).to_bytes())
    }

    fn sticker_message(client: &Client, id: i32, set: i64) -> Message {
        let mut m = message(client, id);
        m.media = Some(Media::Sticker(sticker(set)));
        m
    }

    fn set_name(m: &Message) -> Option<&str> {
        match &m.media {
            Some(Media::Sticker(s)) => s.set_name.as_deref(),
            _ => panic!("not a sticker"),
        }
    }

    #[tokio::test]
    async fn fetch_off_sends_nothing_and_parses_identically() {
        let (client, mut sent) = client(Fetch::default(), false, Vec::new());
        let mut m = sticker_message(&client, 9, 10);
        (m.reply_to_message_id, Arc::make_mut(&mut m.chat).is_forum) = (Some(4), true);
        let m = m.prepare().await;
        assert!(sent.try_recv().is_err());
        assert!(m.prefetched.is_none());
        assert_eq!((m.reply_to_message_id, set_name(&m)), (Some(4), None));
    }

    #[tokio::test]
    async fn users_fetch_the_topic() {
        let fetch = Fetch { topics: true, ..Fetch::default() };
        let (client, mut sent) = client(fetch, false, vec![refused("CHANNEL_PRIVATE")]);
        let input =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), input.clone());
        let raw = Message::bare(9, get_peer(get_channel_id(9)), 0);
        let mut m = Message::parse(&client, Box::new(raw), &Entities::default());
        (Arc::make_mut(&mut m.chat).is_forum, m.message_thread_id) = (true, Some(6));
        let m = m.prepare().await;
        assert_eq!(sent.try_recv().unwrap(), messages::GetForumTopicsByID { peer: input, topics: vec![6] }.to_bytes());
        assert!(m.prefetched.is_none());
    }

    #[tokio::test]
    async fn fetch_replies_fills_the_slot_before_handlers() {
        let replies = found(vec![Message::bare(4, get_peer(-5), 0), Message::bare(5, get_peer(-5), 0)]);
        let (client, mut sent) = client(REPLIES, false, vec![replies]);
        let mut reply = message(&client, 9);
        reply.reply_to_message_id = Some(4);
        let mut pin = message(&client, 10);
        pin.service = Some(ServiceAction::PinnedMessage(5));
        let mut batch = [reply, pin];
        client.prefetch(&mut batch).await;
        // The reply and the pinned message go in one request.
        assert_eq!(sent.try_recv().unwrap(), get_messages(&[4, 5]));

        let [reply, pin] = batch;
        assert_eq!(reply.reply_to_message().await.unwrap().map(|m| m.id), Some(4));
        assert_eq!(pin.pinned_message().await.unwrap().map(|m| m.id), Some(5));
        assert!(sent.try_recv().is_err(), "a helper asked again");
    }

    #[tokio::test]
    async fn the_handler_hook_prefetches() {
        let (client, mut sent) = client(REPLIES, false, vec![found(vec![Message::bare(4, get_peer(-5), 0)])]);
        let mut m = message(&client, 9);
        m.reply_to_message_id = Some(4);
        let m = m.prepare().await;
        assert_eq!(sent.try_recv().unwrap(), get_messages(&[4]));
        assert_eq!(m.reply_to_message().await.unwrap().map(|m| m.id), Some(4));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_gone_reply_answers_none_without_asking_again() {
        let (client, mut sent) = client(REPLIES, false, vec![found(Vec::new())]);
        let mut m = message(&client, 9);
        m.reply_to_message_id = Some(4);
        let m = m.prepare().await;
        sent.try_recv().unwrap();
        assert!(m.reply_to_message().await.unwrap().is_none());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_cached_reply_needs_no_request() {
        let (client, mut sent) = client(REPLIES, false, Vec::new());
        let cached = base::Message::Message(Box::new(Message::bare(4, get_peer(-5), 0)));
        client.0.caches.store(&cached, &Arc::default());
        let mut m = message(&client, 9);
        m.reply_to_message_id = Some(4);
        let m = m.prepare().await;
        assert!(sent.try_recv().is_err());
        assert_eq!(m.prefetched.unwrap().reply.unwrap().map(|m| m.id), Some(4));
    }

    #[tokio::test]
    async fn a_fetch_error_leaves_the_slot_and_the_handler_runs() {
        let (client, mut sent) = client(REPLIES, false, vec![refused("CHANNEL_PRIVATE")]);
        let mut m = message(&client, 9);
        m.reply_to_message_id = Some(4);
        let m = m.prepare().await;
        sent.try_recv().unwrap();
        assert_eq!(m.id, 9);
        assert!(m.prefetched.is_none(), "an error must leave the helper to ask on demand");
    }

    #[tokio::test]
    async fn the_fetched_reply_has_no_reply_of_its_own() {
        let mut replied = Message::bare(4, get_peer(-5), 0);
        let header = types::MessageReplyHeader { reply_to_msg_id: Some(3), ..header() };
        replied.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)));
        let (client, mut sent) = client(REPLIES, false, vec![found(vec![replied])]);
        let mut m = message(&client, 9);
        m.reply_to_message_id = Some(4);
        let m = m.prepare().await;
        assert_eq!(sent.try_recv().unwrap(), get_messages(&[4]));
        assert!(sent.try_recv().is_err(), "the reply's own reply was fetched");
        let replied = m.prefetched.unwrap().reply.unwrap().unwrap();
        assert_eq!(replied.reply_to_message_id, Some(3));
        assert!(replied.prefetched.is_none());
    }

    #[tokio::test]
    async fn bots_skip_topics_and_stories() {
        let fetch = Fetch { replies: false, topics: true, stories: true, stickers: false };
        let (client, mut sent) = client(fetch, true, Vec::new());
        let mut m = message(&client, 9);
        Arc::make_mut(&mut m.chat).is_forum = true;
        (m.reply_to_story_id, m.reply_to_story_user_id) = (Some(4), Some(7));
        let m = m.prepare().await;
        assert!(sent.try_recv().is_err());
        assert!(m.prefetched.is_none());
    }

    #[tokio::test]
    async fn business_messages_get_only_sticker_names() {
        let fetch = Fetch { replies: true, topics: true, stories: true, stickers: true };
        let (client, mut sent) = client(fetch, false, vec![sticker_set(10, "cats")]);
        let mut m = sticker_message(&client, 9, 10);
        m.business_connection_id = Some("conn".into());
        (m.reply_to_message_id, Arc::make_mut(&mut m.chat).is_forum) = (Some(4), true);
        (m.reply_to_story_id, m.reply_to_story_user_id) = (Some(4), Some(7));
        let m = m.prepare().await;
        let request = messages::GetStickerSet { stickerset: set_id(10), hash: 0 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert!(sent.try_recv().is_err());
        assert_eq!(set_name(&m), Some("cats"));
    }

    #[tokio::test]
    async fn skipped_story_media_is_refetched() {
        let fetch = Fetch { replies: false, topics: false, stories: true, stickers: false };
        let (client, mut sent) = client(fetch, false, vec![refused("PEER_ID_INVALID")]);
        let me = base::Peer::PeerUser(types::PeerUser { user_id: 0 });
        let mut story = Story::from_media(&client, &me, 4, &Entities::default());
        story.is_skipped = true;
        let mut m = message(&client, 9);
        m.media = Some(Media::Story(Box::new(story)));
        let m = m.prepare().await;
        let fetch = stories::GetStoriesByID { peer: base::InputPeer::InputPeerSelf, id: vec![4] };
        assert_eq!(sent.try_recv().unwrap(), fetch.to_bytes());
        // The failed fetch leaves the media as it came.
        assert!(matches!(&m.media, Some(Media::Story(s)) if s.id == 4 && s.is_skipped));
    }

    #[tokio::test]
    async fn one_get_sticker_set_per_set_then_the_name_cache() {
        let fetch = Fetch { replies: false, topics: false, stories: false, stickers: true };
        let (client, mut sent) = client(fetch, true, vec![sticker_set(10, "cats")]);
        let mut batch = [sticker_message(&client, 1, 10), sticker_message(&client, 2, 10)];
        client.prefetch(&mut batch).await;
        let request = messages::GetStickerSet { stickerset: set_id(10), hash: 0 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert!(sent.try_recv().is_err(), "one request per set");
        assert_eq!(batch.iter().map(set_name).collect::<Vec<_>>(), [Some("cats"), Some("cats")]);

        let later = sticker_message(&client, 3, 10).prepare().await;
        assert!(sent.try_recv().is_err(), "the name cache answers");
        assert_eq!(set_name(&later), Some("cats"));
    }

    #[tokio::test]
    async fn a_flood_wait_is_handed_back_not_slept() {
        tokio::time::pause();
        let fetch = Fetch { stickers: true, ..Fetch::default() };
        let flood = Err(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_30")));
        let (client, mut sent) = client(fetch, true, vec![flood]);
        let start = Instant::now();
        let m = sticker_message(&client, 1, 10).prepare().await;
        assert!(start.elapsed() < Duration::from_secs(30), "the handler waited for the flood");
        sent.try_recv().unwrap();
        assert!(sent.try_recv().is_err(), "the flooded request went out again");
        assert_eq!(set_name(&m), None);
    }

    #[tokio::test]
    async fn sets_are_looked_up_side_by_side() {
        let fetch = Fetch { stickers: true, ..Fetch::default() };
        let (client, mut sent) = client(fetch, true, vec![sticker_set(10, "cats"), sticker_set(11, "dogs")]);
        let mut batch = [sticker_message(&client, 1, 10), sticker_message(&client, 2, 11)];
        client.prefetch(&mut batch).await;
        let mut asked = [sent.try_recv().unwrap(), sent.try_recv().unwrap()];
        asked.sort();
        let ask = |id| messages::GetStickerSet { stickerset: set_id(id), hash: 0 }.to_bytes();
        let mut want = [ask(10), ask(11)];
        want.sort();
        assert_eq!(asked, want);
        let mut names: Vec<_> = batch.iter().map(set_name).collect();
        names.sort();
        assert_eq!(names, [Some("cats"), Some("dogs")]);
    }

    #[tokio::test]
    async fn stickerset_invalid_leaves_no_name() {
        let fetch = Fetch { replies: false, topics: false, stories: false, stickers: true };
        let (client, mut sent) = client(fetch, true, vec![refused("STICKERSET_INVALID")]);
        let m = sticker_message(&client, 1, 10).prepare().await;
        sent.try_recv().unwrap();
        assert_eq!(set_name(&m), None);
        assert!(client.0.caches.set_name(10).is_none());
    }

    fn replying(id: i32, to: i32) -> types::Message {
        let mut raw = Message::bare(id, get_peer(-5), 0);
        let header = types::MessageReplyHeader { reply_to_msg_id: Some(to), ..header() };
        raw.reply_to = Some(base::MessageReplyHeader::MessageReplyHeader(Box::new(header)));
        raw
    }

    fn history_page() -> Vec<types::Message> {
        vec![replying(12, 1), replying(11, 2), Message::bare(10, get_peer(-5), 0)]
    }

    #[tokio::test]
    async fn a_history_page_fetches_its_replies_in_one_request() {
        let replied = vec![Message::bare(1, get_peer(-5), 0), Message::bare(2, get_peer(-5), 0)];
        let (client, mut sent) = client(REPLIES, false, vec![found(history_page()), found(replied)]);
        let mut history = client.get_chat_history(-5).limit(3);
        let mut read = Vec::new();
        while let Some(m) = history.next().await.unwrap() {
            read.push(m);
        }
        sent.try_recv().unwrap();
        assert_eq!(sent.try_recv().unwrap(), get_messages(&[1, 2]));
        assert!(sent.try_recv().is_err(), "one getMessages for the page");
        let mut replies = Vec::new();
        for m in &read {
            replies.push(m.reply_to_message().await.unwrap().map(|r| r.id));
        }
        assert_eq!(replies, [Some(1), Some(2), None]);
        assert!(sent.try_recv().is_err(), "a reply was fetched again");
    }

    #[tokio::test]
    async fn fetch_off_history_sends_one_request_per_page() {
        let (client, mut sent) = client(Fetch::default(), false, vec![found(history_page())]);
        let mut history = client.get_chat_history(-5).limit(3);
        let mut count = 0;
        while history.next().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 3);
        sent.try_recv().unwrap();
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn get_messages_prefetches() {
        let (client, mut sent) =
            client(REPLIES, false, vec![found(vec![replying(9, 4)]), found(vec![Message::bare(4, get_peer(-5), 0)])]);
        let got = client.get_messages(-5, &[9]).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), get_messages(&[9]));
        assert_eq!(sent.try_recv().unwrap(), get_messages(&[4]));
        let [Some(m)] = got.as_slice() else { panic!("{got:?}") };
        assert_eq!(m.reply_to_message().await.unwrap().map(|r| r.id), Some(4));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn topics_and_stories_batch_per_chat() {
        let fetch = Fetch { topics: true, stories: true, ..Fetch::default() };
        let (client, mut sent) = client(fetch, false, vec![refused("CHANNEL_PRIVATE"), refused("PEER_ID_INVALID")]);
        let input =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(get_channel_id(9), input.clone());
        let mut batch: Vec<Message> = [(1, 6, 4), (2, 6, 5), (3, 7, 4)]
            .into_iter()
            .map(|(id, thread, story)| {
                let raw = Message::bare(id, get_peer(get_channel_id(9)), 0);
                let mut m = Message::parse(&client, Box::new(raw), &crate::dispatcher::Entities::default());
                (Arc::make_mut(&mut m.chat).is_forum, m.message_thread_id) = (true, Some(thread));
                (m.reply_to_story_id, m.reply_to_story_user_id) = (Some(story), Some(0));
                m
            })
            .collect();
        client.prefetch(&mut batch).await;
        // One request per chat for each kind; the two kinds go out together, in either order.
        let topics = messages::GetForumTopicsByID { peer: input, topics: vec![6, 7] }.to_bytes();
        let stories = stories::GetStoriesByID { peer: base::InputPeer::InputPeerSelf, id: vec![4, 5] }.to_bytes();
        let mut got = [sent.try_recv().unwrap(), sent.try_recv().unwrap()];
        got.sort();
        let mut want = [topics, stories];
        want.sort();
        assert_eq!(got, want);
        assert!(sent.try_recv().is_err());
    }
}
