use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use super::Entities;
use super::tasks::Tasks;
use crate::Client;
use crate::client::Me;
use crate::filters::{Facts, Filter, Inspect, PeerKinds, Subject};
use crate::handlers::{Callback, ErrorCallback, RawCallback, Routed, UpdateKind};
use crate::raw::base;
use crate::types::{BotSubscriptionUpdated, CallbackQuery, ChatJoinRequest, GuestMessage, InlineQuery, Message};

type Scan = (Vec<usize>, Option<usize>);

pub(crate) struct Route<T> {
    group: i32,
    stop: bool,
    id: u32,
    filter: Filter<T>,
    callback: Callback<T>,
}

impl<T> Clone for Route<T> {
    fn clone(&self) -> Self {
        let (filter, callback) = (self.filter.clone(), self.callback.clone());
        Route { group: self.group, stop: self.stop, id: self.id, filter, callback }
    }
}

pub(crate) struct Handlers<T>(Vec<Route<T>>);

impl<T> Default for Handlers<T> {
    fn default() -> Self {
        Handlers(Vec::new())
    }
}

impl<T> Clone for Handlers<T> {
    fn clone(&self) -> Self {
        Handlers(self.0.clone())
    }
}

impl<T: Routed + Subject + Inspect> Handlers<T> {
    pub(crate) fn push(&mut self, id: u32, filter: Filter<T>, callback: Callback<T>) {
        self.insert(Route { group: 0, stop: false, id, filter, callback });
    }

    fn insert(&mut self, route: Route<T>) {
        let at = self.0.partition_point(|r| (r.group, r.id) < (route.group, route.id));
        self.0.insert(at, route);
    }

    fn configure(&mut self, id: u32, group: Option<i32>, stop: bool) {
        if let Some(at) = self.0.iter().position(|r| r.id == id) {
            let mut route = self.0.remove(at);
            route.group = group.unwrap_or(route.group);
            route.stop |= stop;
            self.insert(route);
        }
    }

    fn walk(&self, start: usize, matched: &mut Vec<usize>, test: impl Fn(&Filter<T>) -> Option<bool>) -> Option<usize> {
        let mut group = matched.last().map(|&i| self.0[i].group);
        for (i, route) in self.0.iter().enumerate().skip(start) {
            if group == Some(route.group) {
                continue;
            }
            match test(&route.filter) {
                None => return Some(i),
                Some(false) => continue,
                Some(true) => matched.push(i),
            }
            if route.stop {
                break;
            }
            group = Some(route.group);
        }
        None
    }

    /// Walks the raw update's facts. `None` when nothing can match, so the update is never parsed; a miss
    /// allocates nothing.
    fn scan(&self, facts: &Facts<'_>) -> Option<Scan> {
        let mut matched = Vec::new();
        let resume = self.walk(0, &mut matched, |f| f.decide(facts));
        (!matched.is_empty() || resume.is_some()).then_some((matched, resume))
    }

    fn finish(&self, kind: UpdateKind, tasks: &mut Tasks, me: &Me, (mut matched, resume): Scan, item: T) {
        if let Some(start) = resume {
            let facts = item.facts(me);
            self.walk(start, &mut matched, |f| Some(f.eval(&facts, &item)));
        }
        if !matched.is_empty() {
            tasks.run(kind, item, matched.iter().map(|&i| self.0[i].callback.clone()).collect());
        }
    }
}

impl Handlers<Message> {
    fn run(&self, kind: UpdateKind, tasks: &mut Tasks, client: &Client, raw: base::Message, entities: &Entities) {
        if self.0.is_empty() {
            return;
        }
        let me = client.0.me.lock().unwrap().clone();
        let Some(scan) = self.scan(&Facts::message(&raw, PeerKinds::message(&raw, entities), &me)) else { return };
        if let Some(message) = Message::from_raw(client, raw, entities) {
            self.finish(kind, tasks, &me, scan, message);
        }
    }
}

#[derive(Default, Clone)]
pub(crate) struct Router {
    pub(crate) message: Handlers<Message>,
    pub(crate) edited_message: Handlers<Message>,
    pub(crate) callback_query: Handlers<CallbackQuery>,
    pub(crate) inline_query: Handlers<InlineQuery>,
    pub(crate) chat_join_request: Handlers<ChatJoinRequest>,
    pub(crate) guest_message: Handlers<GuestMessage>,
    pub(crate) bot_subscription_updated: Handlers<BotSubscriptionUpdated>,
    pub(crate) raw: Vec<RawCallback>,
    pub(crate) on_error: Option<ErrorCallback>,
    next_id: u32,
}

impl Router {
    pub(crate) fn next_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub(crate) fn configure(&mut self, kind: UpdateKind, id: u32, group: Option<i32>, stop: bool) {
        match kind {
            UpdateKind::Message => self.message.configure(id, group, stop),
            UpdateKind::EditedMessage => self.edited_message.configure(id, group, stop),
            UpdateKind::CallbackQuery => self.callback_query.configure(id, group, stop),
            UpdateKind::InlineQuery => self.inline_query.configure(id, group, stop),
            UpdateKind::ChatJoinRequest => self.chat_join_request.configure(id, group, stop),
            UpdateKind::GuestMessage => self.guest_message.configure(id, group, stop),
            UpdateKind::BotSubscriptionUpdated => self.bot_subscription_updated.configure(id, group, stop),
            UpdateKind::Raw => {}
        }
    }

    pub(crate) fn dispatch(&self, client: &Client, update: base::Update, entities: &Entities, tasks: &mut Tasks) {
        use base::Update as U;
        for callback in &self.raw {
            tasks.raw(callback(client.clone(), update.clone()));
        }
        let me = || client.0.me.lock().unwrap().clone();
        match update {
            U::UpdateNewMessage(u) => self.message.run(UpdateKind::Message, tasks, client, u.message, entities),
            U::UpdateNewChannelMessage(u) => self.message.run(UpdateKind::Message, tasks, client, u.message, entities),
            U::UpdateEditMessage(u) => {
                self.edited_message.run(UpdateKind::EditedMessage, tasks, client, u.message, entities)
            }
            U::UpdateEditChannelMessage(u) => {
                self.edited_message.run(UpdateKind::EditedMessage, tasks, client, u.message, entities)
            }
            U::UpdateBotCallbackQuery(u) if !self.callback_query.0.is_empty() => {
                let (me, kinds) = (me(), PeerKinds::callback(&u, entities));
                if let Some(scan) = self.callback_query.scan(&Facts::callback(&u, kinds, &me)) {
                    let query = CallbackQuery::parse(client, &u, entities);
                    self.callback_query.finish(UpdateKind::CallbackQuery, tasks, &me, scan, query);
                }
            }
            U::UpdateBotInlineQuery(u) if !self.inline_query.0.is_empty() => {
                let query = InlineQuery::parse(client, &u, entities);
                self.inline_query.finish(UpdateKind::InlineQuery, tasks, &me(), (Vec::new(), Some(0)), query);
            }
            U::UpdateBotChatInviteRequester(u) if !self.chat_join_request.0.is_empty() => {
                let item = ChatJoinRequest::parse(client, &u, entities);
                self.chat_join_request.finish(UpdateKind::ChatJoinRequest, tasks, &me(), (Vec::new(), Some(0)), item);
            }
            U::UpdateBotGuestChatQuery(u) if !self.guest_message.0.is_empty() => {
                if let Some(item) = GuestMessage::parse(client, *u, entities) {
                    self.guest_message.finish(UpdateKind::GuestMessage, tasks, &me(), (Vec::new(), Some(0)), item);
                }
            }
            U::UpdateBotStarsSubscription(u) if !self.bot_subscription_updated.0.is_empty() => {
                if let Some(item) = BotSubscriptionUpdated::parse(client, &u, entities) {
                    let kind = UpdateKind::BotSubscriptionUpdated;
                    self.bot_subscription_updated.finish(kind, tasks, &me(), (Vec::new(), Some(0)), item);
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct Routes {
    router: RwLock<Arc<Router>>,
    generation: AtomicU64,
}

impl Routes {
    pub(crate) fn add<R>(&self, f: impl FnOnce(&mut Router) -> R) -> R {
        let mut router = self.router.write().unwrap();
        let result = f(Arc::make_mut(&mut router));
        self.generation.fetch_add(1, Ordering::Release);
        result
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn current(&self) -> Arc<Router> {
        self.router.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use tokio::sync::Semaphore;

    use super::*;
    use crate::errors::Error;
    use crate::filters;
    use crate::handlers::callback;
    use crate::raw::types;
    use crate::utils::get_peer;

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn record<T: Send + 'static>(log: &Log, name: &'static str) -> Callback<T> {
        let log = log.clone();
        callback(move |_: T| {
            log.lock().unwrap().push(name);
            async { Ok(()) }
        })
    }

    fn fail<T: Send + 'static>(log: &Log, name: &'static str) -> Callback<T> {
        let log = log.clone();
        callback(move |_: T| {
            log.lock().unwrap().push(name);
            async { Err::<(), _>(Error::Unexpected("guard")) }
        })
    }

    fn new_message(id: i32, out: bool) -> base::Update {
        let mut message = Message::bare(id, get_peer(1), 0);
        (message.out, message.message) = (out, "/start".into());
        let message = base::Message::Message(Box::new(message));
        base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
    }

    fn inline_query() -> base::Update {
        base::Update::UpdateBotInlineQuery(Box::new(types::UpdateBotInlineQuery {
            query_id: 1,
            user_id: 1,
            query: "hi".into(),
            geo: None,
            peer_type: None,
            offset: String::new(),
        }))
    }

    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    fn dispatch(router: &Router, update: base::Update) {
        let mut tasks = Tasks::default();
        tasks.on_error = router.on_error.clone();
        router.dispatch(&Client::offline(), update, &Entities::default(), &mut tasks);
    }

    fn router_of(client: &Client) -> Arc<Router> {
        client.0.handlers.current()
    }

    #[tokio::test]
    async fn routes_each_kind_to_its_own_list() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::all(), record(&log, "message"));
        router.edited_message.push(2, filters::all(), record(&log, "edited"));
        router.inline_query.push(3, filters::all(), record(&log, "inline"));
        let raw = log.clone();
        router.raw.push(Arc::new(move |_, _| {
            raw.lock().unwrap().push("raw");
            Box::pin(async { Ok(()) })
        }));
        dispatch(&router, new_message(1, false));
        dispatch(&router, inline_query());
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["inline", "message", "raw", "raw"]);
    }

    #[tokio::test]
    async fn first_match_in_a_group_wins() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::outgoing(), record(&log, "outgoing"));
        router.message.push(2, filters::command("start"), record(&log, "start"));
        router.message.push(3, filters::all(), record(&log, "all"));
        dispatch(&router, new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["start"]);
    }

    #[tokio::test]
    async fn every_group_gets_a_turn_in_ascending_order() {
        let (client, log) = (Client::offline(), Log::default());
        let (late, early, first) = (log.clone(), log.clone(), log.clone());
        client
            .on_message(filters::all(), move |_| {
                late.lock().unwrap().push("group 1");
                async { Ok(()) }
            })
            .group(1);
        client.on_message(filters::command("start"), move |_| {
            first.lock().unwrap().push("group 0");
            async { Ok(()) }
        });
        client
            .on_message(filters::all(), move |_| {
                early.lock().unwrap().push("group -1");
                async { Ok(()) }
            })
            .group(-1);
        dispatch(&router_of(&client), new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["group -1", "group 0", "group 1"]);
    }

    #[tokio::test]
    async fn stop_keeps_later_groups_out() {
        let (client, log) = (Client::offline(), Log::default());
        let (guard, rest, other) = (log.clone(), log.clone(), log.clone());
        client.on_message(filters::all(), move |_| {
            rest.lock().unwrap().push("handler");
            async { Ok(()) }
        });
        client
            .on_message(filters::outgoing(), move |_| {
                other.lock().unwrap().push("outgoing");
                async { Ok(()) }
            })
            .group(-2)
            .stop();
        client
            .on_message(filters::command("start"), move |_| {
                guard.lock().unwrap().push("guard");
                async { Ok(()) }
            })
            .stop()
            .group(-1);
        dispatch(&router_of(&client), new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["guard"]);
    }

    #[tokio::test]
    async fn an_error_stops_later_groups_and_reaches_on_error() {
        let (client, log) = (Client::offline(), Log::default());
        let errors = Arc::new(Mutex::new(Vec::new()));
        let seen = errors.clone();
        client.on_error(move |e| {
            seen.lock().unwrap().push((e.kind, e.chat_id, e.error.to_string()));
            async {}
        });
        client.0.handlers.add(|r| {
            let (a, b) = (r.next_id(), r.next_id());
            r.message.push(a, filters::all(), fail(&log, "guard"));
            r.message.push(b, filters::all(), record(&log, "late"));
            r.configure(UpdateKind::Message, b, Some(1), false);
        });
        let failing = log.clone();
        client.on_raw_update(move |_, _| {
            failing.lock().unwrap().push("raw");
            async { Err::<(), _>(Error::Unexpected("raw")) }
        });
        dispatch(&router_of(&client), new_message(1, false));
        settle().await;
        let mut ran = log.lock().unwrap().clone();
        ran.sort_unstable();
        assert_eq!(ran, ["guard", "raw"]);
        let mut errors = errors.lock().unwrap().clone();
        errors.sort_by_key(|e| e.1);
        let guard = Error::Unexpected("guard").to_string();
        let raw = Error::Unexpected("raw").to_string();
        assert_eq!(errors, [(UpdateKind::Raw, None, raw), (UpdateKind::Message, Some(1), guard)]);
    }

    async fn second_finishes_first(ordered: bool) -> bool {
        let (client, log) = (Client::offline(), Log::default());
        let gate = Arc::new(tokio::sync::Notify::new());
        let (wait, done) = (gate.clone(), log.clone());
        client.on_message(filters::all(), move |m| {
            let (wait, done) = (wait.clone(), done.clone());
            async move {
                if m.id == 1 {
                    wait.notified().await;
                }
                done.lock().unwrap().push(if m.id == 1 { "first" } else { "second" });
                Ok(())
            }
        });
        let (router, mut tasks) = (router_of(&client), Tasks::new(ordered, None));
        for id in [1, 2] {
            router.dispatch(&client, new_message(id, false), &Entities::default(), &mut tasks);
        }
        settle().await;
        gate.notify_one();
        settle().await;
        let log = log.lock().unwrap();
        assert_eq!(log.len(), 2);
        log[0] == "second"
    }

    #[tokio::test]
    async fn ordered_runs_a_chat_in_arrival_order() {
        assert!(second_finishes_first(false).await);
        assert!(!second_finishes_first(true).await);
    }

    #[tokio::test]
    async fn a_panicking_handler_does_not_wedge_its_chat() {
        let (client, log) = (Client::offline(), Log::default());
        let done = log.clone();
        client.on_message(filters::all(), move |m| {
            let done = done.clone();
            async move {
                assert!(m.id != 1, "boom");
                done.lock().unwrap().push("second");
                Ok(())
            }
        });
        let (router, mut tasks) = (router_of(&client), Tasks::new(true, None));
        for id in [1, 2] {
            router.dispatch(&client, new_message(id, false), &Entities::default(), &mut tasks);
        }
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["second"]);
    }

    #[tokio::test]
    async fn the_cap_bounds_running_handlers() {
        let client = Client::offline();
        let (running, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (now, max) = (running.clone(), peak.clone());
        client.on_message(filters::all(), move |_| {
            let (now, max) = (now.clone(), max.clone());
            async move {
                max.fetch_max(now.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                tokio::task::yield_now().await;
                now.fetch_sub(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let router = router_of(&client);
        let mut tasks = Tasks::new(false, Some(Arc::new(Semaphore::new(2))));
        for id in 0..6 {
            router.dispatch(&client, new_message(id, false), &Entities::default(), &mut tasks);
        }
        for _ in 0..10 {
            settle().await;
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        assert_eq!(running.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn closures_decide_on_the_parsed_message_in_their_group() {
        let log = Log::default();
        let mut router = Router::default();
        router.message.push(1, filters::command("start"), record(&log, "start"));
        router.message.push(2, filters::create(|m: &Message| m.text == "/nope"), record(&log, "nope"));
        router.message.push(3, filters::private() & filters::create(|m: &Message| m.id == 1), record(&log, "one"));
        router.message.push(4, filters::all(), record(&log, "all"));
        router.configure(UpdateKind::Message, 2, Some(1), false);
        router.configure(UpdateKind::Message, 3, Some(1), false);
        router.configure(UpdateKind::Message, 4, Some(1), false);
        dispatch(&router, new_message(1, false));
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["start", "one"]);
    }

    #[test]
    fn message_filters_work_on_other_kinds() {
        let client = Client::offline();
        let entities = Entities::default();
        let base::Update::UpdateBotInlineQuery(raw) = inline_query() else { unreachable!() };
        let query = InlineQuery::parse(&client, &raw, &entities);
        assert_eq!((filters::text().check(&query), filters::user(1).check(&query)), (true, true));
        let base::Update::UpdateNewMessage(raw) = new_message(1, false) else { unreachable!() };
        let base::Message::Message(raw) = &raw.message else { unreachable!() };
        let message = Message::parse(&client, raw.clone(), &entities);
        assert_eq!((filters::text().check(&message), filters::private().check(&message)), (true, true));
    }
}
