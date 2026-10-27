use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use super::Entities;
use crate::Client;
use crate::filters::Filter;
use crate::handlers::{BoxFuture, Callback, RawCallback};
use crate::raw::base;
use crate::types::{CallbackQuery, InlineQuery, Message};

pub(crate) struct Handlers<T>(Vec<(Filter<T>, Callback<T>)>);

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

impl<T> Handlers<T> {
    pub(crate) fn push(&mut self, filter: Filter<T>, callback: Callback<T>) {
        self.0.push((filter, callback));
    }

    /// Parses only when this kind has handlers; the first one whose filter matches gets the value.
    fn run(&self, parse: impl FnOnce() -> Option<T>) -> Option<BoxFuture> {
        if self.0.is_empty() {
            return None;
        }
        let value = parse()?;
        let (_, callback) = self.0.iter().find(|(f, _)| f.check(&value))?;
        Some(callback(value))
    }
}

/// One list per update kind. A new kind adds a field here and an arm in `dispatch`.
#[derive(Default, Clone)]
pub(crate) struct Router {
    pub(crate) message: Handlers<Message>,
    pub(crate) edited_message: Handlers<Message>,
    pub(crate) callback_query: Handlers<CallbackQuery>,
    pub(crate) inline_query: Handlers<InlineQuery>,
    pub(crate) raw: Vec<RawCallback>,
}

impl Router {
    pub(crate) fn dispatch(&self, client: &Client, update: base::Update, entities: &Entities) {
        use base::Update as U;
        let message = |m: base::Message| match m {
            base::Message::Message(m) => Some(Message::parse(client, m, entities)),
            _ => None,
        };
        // Raw handlers get copies, so the typed arm below can move the update's parts.
        for callback in &self.raw {
            spawn(callback(client.clone(), update.clone()));
        }
        let future = match update {
            U::UpdateNewMessage(u) => self.message.run(move || message(u.message)),
            U::UpdateNewChannelMessage(u) => self.message.run(move || message(u.message)),
            U::UpdateEditMessage(u) => self.edited_message.run(move || message(u.message)),
            U::UpdateEditChannelMessage(u) => self.edited_message.run(move || message(u.message)),
            U::UpdateBotCallbackQuery(u) => {
                self.callback_query.run(|| Some(CallbackQuery::parse(client, &u, entities)))
            }
            U::UpdateBotInlineQuery(u) => self.inline_query.run(|| Some(InlineQuery::parse(client, &u, entities))),
            _ => None,
        };
        if let Some(future) = future {
            spawn(future);
        }
    }
}

#[derive(Default)]
pub(crate) struct Routes {
    router: RwLock<Arc<Router>>,
    generation: AtomicU64,
}

impl Routes {
    pub(crate) fn add(&self, f: impl FnOnce(&mut Router)) {
        let mut router = self.router.write().unwrap();
        f(Arc::make_mut(&mut router));
        self.generation.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) fn current(&self) -> Arc<Router> {
        self.router.read().unwrap().clone()
    }
}

fn spawn(future: BoxFuture) {
    tokio::spawn(async move {
        if let Err(e) = future.await {
            eprintln!("vaegram: handler failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use super::{Arc, Callback, Client, Entities, Handlers, InlineQuery, Message, Ordering, Router, base};
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
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn routes_each_kind_to_its_own_list() {
        let (client, log) = (Client::offline(), Log::default());
        let mut router = Router::default();
        router.message.push(filters::all(), record(&log, "message"));
        router.edited_message.push(filters::all(), record(&log, "edited"));
        router.inline_query.push(filters::all(), record(&log, "inline"));
        let raw = log.clone();
        router.raw.push(Arc::new(move |_, _| {
            raw.lock().unwrap().push("raw");
            Box::pin(async { Ok(()) })
        }));
        let entities = Entities::default();
        router.dispatch(&client, new_message(1, false), &entities);
        router.dispatch(&client, inline_query(), &entities);
        settle().await;
        let mut seen = log.lock().unwrap().clone();
        seen.sort_unstable();
        assert_eq!(seen, ["inline", "message", "raw", "raw"]);
    }

    #[tokio::test]
    async fn first_matching_handler_wins() {
        let (client, log) = (Client::offline(), Log::default());
        let mut router = Router::default();
        router.message.push(filters::outgoing(), record(&log, "outgoing"));
        router.message.push(filters::command("start"), record(&log, "start"));
        router.message.push(filters::all(), record(&log, "all"));
        router.dispatch(&client, new_message(1, false), &Entities::default());
        settle().await;
        assert_eq!(*log.lock().unwrap(), ["start"]);
    }

    #[test]
    fn parses_only_when_the_kind_has_handlers() {
        let parses = AtomicUsize::new(0);
        let parse = || {
            parses.fetch_add(1, Ordering::Relaxed);
            Some(())
        };
        let mut handlers = Handlers::<()>::default();
        assert_eq!((handlers.run(parse).is_none(), parses.load(Ordering::Relaxed)), (true, 0));
        handlers.push(filters::create(|_| false), callback(|_| async { Ok(()) }));
        assert_eq!((handlers.run(parse).is_none(), parses.load(Ordering::Relaxed)), (true, 1));
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
