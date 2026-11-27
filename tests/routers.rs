//! Routers, middleware and provided values, driven through the public test kit as a bot's own tests would.

use std::future::{Ready, ready};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vaegram::enums::ChatAction;
use vaegram::filters;
use vaegram::handlers::{
    CallbackAnswer, Dep, FromContext, MissingDependency, Next, Router, UpdateContext, UpdateKind,
    answer_callback_queries, chat_action,
};
use vaegram::test::{CallKind, Recorder, TestCallbackQuery, TestChat, TestMessage, TestUser};
use vaegram::types::{CallbackQuery, Message};
use vaegram::{Client, Error, Result};

type Log = Arc<Mutex<Vec<String>>>;

/// Runs `f` and fails if it takes 5 seconds: a test client answers at once.
async fn soon<T>(f: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), f).await.expect("it hung")
}

/// A handler that writes `name` to `log`.
fn note<T: 'static>(log: &Log, name: &'static str) -> impl Fn(T) -> Ready<Result<()>> + Send + Sync + 'static {
    let log = log.clone();
    move |_| {
        log.lock().unwrap().push(name.to_owned());
        ready(Ok(()))
    }
}

fn push(log: &Log, entry: impl Into<String>) {
    log.lock().unwrap().push(entry.into());
}

/// What `log` holds, emptied.
fn take(log: &Log) -> Vec<String> {
    std::mem::take(&mut *log.lock().unwrap())
}

async fn feed(recorder: &Recorder, text: &str) {
    soon(recorder.feed(TestMessage::private(42, text))).await;
}

#[tokio::test]
async fn nested_routers_run_their_own_groups_inside_their_parents_turn() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut inner = Router::new();
    inner.on_message(filters::all(), note(&log, "inner"));
    let mut outer = Router::new();
    outer.on_message(filters::command("x"), note(&log, "outer"));
    outer.on_message(filters::all(), note(&log, "outer late")).group(1);
    outer.include_router(inner).group(-1);
    client.include_router(outer);
    client.on_message(filters::all(), note(&log, "client")).group(1);

    feed(&recorder, "/x").await;
    assert_eq!(take(&log), ["inner", "outer", "outer late", "client"]);
}

#[tokio::test]
async fn the_first_match_in_a_group_wins_between_handlers_and_routers() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    client.on_message(filters::command("a"), note(&log, "client a"));
    let mut router = Router::new();
    router.on_message(filters::command(["a", "b", "c"]), note(&log, "router"));
    client.include_router(router).stop();
    client.on_message(filters::all(), note(&log, "same group"));
    client.on_message(filters::all(), note(&log, "next group")).group(1);

    // A handler registered before the router takes the group, and the router never sees the update.
    feed(&recorder, "/a").await;
    assert_eq!(take(&log), ["client a", "next group"]);
    // The router takes the group, and its `stop` ends the walk.
    feed(&recorder, "/b").await;
    assert_eq!(take(&log), ["router"]);
    // Nothing in the router matches: the update goes on as if the router weren't there.
    feed(&recorder, "hello").await;
    assert_eq!(take(&log), ["same group", "next group"]);
}

#[tokio::test]
async fn an_outer_middleware_sees_updates_before_filters_and_can_keep_them() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut admin = Router::new();
    let seen = log.clone();
    admin.outer_middleware(move |ctx, next| {
        let seen = seen.clone();
        async move {
            push(&seen, format!("gate {:?}", ctx.user_id()));
            if ctx.user_id() == Some(1) { next.run(ctx).await } else { Ok(()) }
        }
    });
    admin.on_message(filters::command("ban"), note(&log, "ban"));
    client.include_router(admin);
    client.on_message(filters::all(), note(&log, "fallback"));

    // Kept: neither the router's handler nor the handler after it in the group runs.
    feed(&recorder, "/ban").await;
    assert_eq!(take(&log), ["gate Some(42)"]);
    soon(recorder.feed(TestMessage::private(1, "/ban"))).await;
    assert_eq!(take(&log), ["gate Some(1)", "ban"]);
    // Let through, and nothing inside matched: the update falls through to the next handler in the group.
    soon(recorder.feed(TestMessage::private(1, "hello"))).await;
    assert_eq!(take(&log), ["gate Some(1)", "fallback"]);
}

#[tokio::test]
async fn inner_middleware_wraps_only_matched_handlers_and_nests_outside_in() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let wrap = |name: &'static str| {
        let log = log.clone();
        move |ctx: UpdateContext, next: Next| {
            let log = log.clone();
            async move {
                push(&log, format!("{name} before"));
                let result = next.run(ctx).await;
                push(&log, format!("{name} after"));
                result
            }
        }
    };
    let mut child = Router::new();
    child.inner_middleware(wrap("child"));
    child.on_message(filters::command("go"), note(&log, "handler"));
    let mut parent = Router::new();
    parent.inner_middleware(wrap("parent"));
    parent.include_router(child);
    client.include_router(parent);

    feed(&recorder, "/stop").await;
    assert!(take(&log).is_empty());
    feed(&recorder, "/go").await;
    assert_eq!(take(&log), ["parent before", "child before", "handler", "child after", "parent after"]);
}

#[tokio::test]
async fn a_middleware_can_change_the_update_for_the_handlers_after_it() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut router = Router::new();
    router.message().outer_middleware(|mut ctx, next| async move {
        assert_eq!(ctx.kind(), UpdateKind::Message);
        if let Some(message) = ctx.event_mut::<Message>() {
            Arc::make_mut(&mut message.chat).id = 7;
        }
        next.run(ctx).await
    });
    let seen = log.clone();
    router.on_message(filters::all(), move |msg| {
        let seen = seen.clone();
        async move {
            push(&seen, format!("chat {}", msg.chat.id));
            Ok(())
        }
    });
    client.include_router(router);

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["chat 7"]);
}

#[tokio::test]
async fn a_routers_on_error_takes_its_handlers_errors_and_stops_later_groups() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let seen = log.clone();
    client.on_error(move |e| {
        push(&seen, format!("client: {}", e.error));
        async {}
    });
    let mut router = Router::new();
    let seen = log.clone();
    router.on_error(move |e| {
        push(&seen, format!("router: {:?} {:?} {:?}", e.kind, e.chat_id, e.error.to_string().contains("boom")));
        async {}
    });
    router.on_message(filters::all(), |_| async { Err::<(), _>(Error::Unexpected("boom")) });
    client.include_router(router);
    client.on_message(filters::all(), note(&log, "later")).group(1);

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["router: Message Some(42) true"]);
}

#[tokio::test]
async fn without_on_error_a_routers_errors_reach_the_clients() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let seen = log.clone();
    client.on_error(move |e| {
        push(&seen, format!("client: {:?} {:?}", e.kind, e.user_id));
        async {}
    });
    let mut inner = Router::new();
    inner.on_message(filters::all(), |_| async { Err::<(), _>(Error::Unexpected("boom")) });
    let mut outer = Router::new();
    outer.include_router(inner);
    client.include_router(outer);
    client.on_message(filters::all(), note(&log, "later")).group(1);

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["client: Message Some(42)"]);
}

#[tokio::test]
async fn a_middleware_that_catches_an_error_lets_later_groups_run() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let seen = log.clone();
    client.on_error(move |_| {
        push(&seen, "reported");
        async {}
    });
    let mut router = Router::new();
    let seen = log.clone();
    router.inner_middleware(move |ctx, next| {
        let seen = seen.clone();
        async move {
            if let Err(e) = next.run(ctx).await {
                push(&seen, format!("caught {}", e.to_string().contains("boom")));
            }
            Ok(())
        }
    });
    router.on_message(filters::all(), |_| async { Err::<(), _>(Error::Unexpected("boom")) });
    client.include_router(router);
    client.on_message(filters::all(), note(&log, "later")).group(1);

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["caught true", "later"]);
}

#[derive(Clone)]
struct AppName(&'static str);

#[derive(Clone)]
struct Locale(&'static str);

#[derive(Clone)]
struct Caller(i64);

#[tokio::test]
async fn handlers_take_values_from_the_client_their_router_and_middleware() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    client.provide(AppName("bot"));
    client.provide(Locale("en"));
    let mut router = Router::new();
    // The router's value wins over the client's.
    router.provide(Locale("de"));
    router.outer_middleware(|mut ctx, next| async move {
        let caller = Caller(ctx.user_id().unwrap_or_default());
        ctx.provide(caller);
        next.run(ctx).await
    });
    let seen = log.clone();
    router.message().handle(
        filters::all(),
        move |msg: Message, Dep(app): Dep<AppName>, Dep(locale): Dep<Locale>, Dep(caller): Dep<Caller>| {
            let seen = seen.clone();
            async move {
                push(&seen, format!("{} {} {} {} {}", msg.text(), app.0, locale.0, caller.0, msg.chat.id));
                Ok::<_, Error>(())
            }
        },
    );
    client.include_router(router);

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["hi bot de 42 42"]);
}

#[derive(Clone)]
struct NeverProvided;

async fn needs_a_value(_msg: Message, Dep(_value): Dep<NeverProvided>) -> Result<()> {
    panic!("ran without its value");
}

async fn may_take_a_value(_msg: Message, value: Option<Dep<NeverProvided>>, client: Client) -> Result<()> {
    assert!(value.is_none());
    client.send_message(42, "ran").await?;
    Ok(())
}

#[tokio::test]
async fn a_missing_value_is_an_error_not_a_panic() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let seen = log.clone();
    client.on_error(move |e| {
        let missing = e.error.downcast_ref::<MissingDependency>().map(|m| m.type_name.ends_with("NeverProvided"));
        push(&seen, format!("{missing:?}"));
        async {}
    });
    let mut router = Router::new();
    router.message().handle(filters::command("needs"), needs_a_value);
    router.message().handle(filters::command("may"), may_take_a_value);
    client.include_router(router);

    feed(&recorder, "/needs").await;
    assert_eq!(take(&log), ["Some(true)"]);
    feed(&recorder, "/may").await;
    assert!(take(&log).is_empty());
    assert_eq!(recorder.sent_texts(42), ["ran"]);
}

#[tokio::test]
async fn a_router_filter_skips_the_whole_router() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut private = Router::new();
    private.message().filter(filters::private());
    let seen = log.clone();
    private.message().outer_middleware(move |ctx, next| {
        let seen = seen.clone();
        async move {
            push(&seen, "outer");
            next.run(ctx).await
        }
    });
    private.on_message(filters::all(), note(&log, "private"));
    client.include_router(private);
    client.on_message(filters::all(), note(&log, "anywhere"));

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["outer", "private"]);
    // A group message fails the router's filter before its outer middleware runs.
    let group = TestMessage::in_chat(TestChat::group(5), TestUser::new(42), "hi");
    soon(recorder.feed(group)).await;
    assert_eq!(take(&log), ["anywhere"]);
}

#[tokio::test]
async fn a_kind_scoped_outer_middleware_sees_that_kind_without_handlers() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut router = Router::new();
    let seen = log.clone();
    router.callback_query().outer_middleware(move |ctx, next| {
        let seen = seen.clone();
        async move {
            let data = ctx.callback_query().map(|q| q.data.clone()).unwrap_or_default();
            push(&seen, format!("outer {data}"));
            next.run(ctx).await
        }
    });
    client.include_router(router);
    client.on_callback_query(filters::all(), note(&log, "client"));
    client.on_message(filters::all(), note(&log, "message"));

    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "tap"))).await;
    assert_eq!(take(&log), ["outer tap", "client"]);
    // The middleware is for button presses alone.
    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["message"]);
}

#[tokio::test]
async fn routes_inside_routers_are_listed_and_removed_by_id() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut router = Router::new();
    let a = router.on_message(filters::command("a"), note(&log, "a")).id();
    let b = router.on_message(filters::command("b"), note(&log, "b")).group(1).id();
    let mounted = client.include_router(router).id();
    // A recording client has no other message handlers.
    let listed: Vec<_> =
        client.handlers().iter().filter(|h| h.kind == UpdateKind::Message).map(|h| (h.id, h.group)).collect();
    assert_eq!(listed, [(a, Some(0)), (b, Some(1))]);

    assert!(client.remove_handler(a));
    assert!(!client.remove_handler(a));
    feed(&recorder, "/a").await;
    feed(&recorder, "/b").await;
    assert_eq!(take(&log), ["b"]);

    assert!(client.remove_handler(mounted));
    assert!(client.handlers().iter().all(|h| h.kind != UpdateKind::Message));
    feed(&recorder, "/b").await;
    assert!(take(&log).is_empty());
}

#[tokio::test]
async fn answer_callback_queries_answers_with_what_the_handler_set() {
    let (client, recorder) = Client::recording();
    let mut router = Router::new();
    router.callback_query().inner_middleware(answer_callback_queries());
    router.callback_query().handle(
        filters::data("save"),
        |_query: CallbackQuery, Dep(answer): Dep<CallbackAnswer>| async move {
            answer.text("Saved").alert();
            Ok::<_, Error>(())
        },
    );
    router.callback_query().handle(
        filters::data("self"),
        |query: CallbackQuery, Dep(answer): Dep<CallbackAnswer>| async move {
            answer.disable();
            query.answer().text("Mine").await
        },
    );
    client.include_router(router);

    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "save").query_id(99))).await;
    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "self").query_id(100))).await;
    let answers: Vec<_> = recorder
        .calls()
        .into_iter()
        .filter_map(|call| match call.kind {
            CallKind::AnswerCallbackQuery { query_id, text, alert, .. } => Some((query_id, text, alert)),
            _ => None,
        })
        .collect();
    assert_eq!(answers, [(99, Some("Saved".to_owned()), true), (100, Some("Mine".to_owned()), false)]);
}

#[tokio::test]
async fn chat_action_shows_while_the_handler_runs() {
    let (client, recorder) = Client::recording();
    let mut router = Router::new();
    router.inner_middleware(chat_action(ChatAction::Typing));
    router.on_message(filters::all(), |msg| async move { msg.reply("done").await });
    client.include_router(router);

    feed(&recorder, "hi").await;
    let methods: Vec<_> = recorder.calls().iter().map(|call| call.method).collect();
    let typing = methods.iter().position(|&m| m == "messages.setTyping");
    let sent = methods.iter().position(|&m| m == "messages.sendMessage");
    assert!(typing.is_some() && sent.is_some() && typing < sent, "{methods:?}");
}

#[tokio::test]
async fn unhandled_updates_still_arrive_past_a_router_with_an_outer_middleware() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let mut router = Router::new();
    router.outer_middleware(|ctx: UpdateContext, next: Next| async move {
        if ctx.message().is_some_and(|m| m.text() == "keep") { Ok(()) } else { next.run(ctx).await }
    });
    router.on_message(filters::command("router"), note(&log, "router"));
    client.include_router(router);
    let seen = log.clone();
    client.on_unhandled_update(filters::all(), move |_| {
        let seen = seen.clone();
        async move {
            push(&seen, "unhandled");
            Ok(())
        }
    });

    // Let through, and nothing in the router matched: nothing took it.
    feed(&recorder, "hello").await;
    assert_eq!(take(&log), ["unhandled"]);
    feed(&recorder, "/router").await;
    assert_eq!(take(&log), ["router"]);
    // Kept by the middleware: taken.
    feed(&recorder, "keep").await;
    assert!(take(&log).is_empty());
}

/// A handler argument of a bot's own, read from the update.
struct Shouted(String);

impl FromContext for Shouted {
    fn from_context(ctx: &UpdateContext) -> Result<Self> {
        let message = ctx.message().ok_or(Error::Unexpected("no message in the context"))?;
        Ok(Shouted(message.text().to_uppercase()))
    }
}

#[tokio::test]
async fn an_argument_of_your_own_reads_the_update() {
    let (client, recorder) = Client::recording();
    let log = Log::default();
    let seen = log.clone();
    let mut router = Router::new();
    router.message().handle(filters::all(), move |_msg: Message, Shouted(text): Shouted| {
        let seen = seen.clone();
        async move {
            push(&seen, text);
            Ok::<_, Error>(())
        }
    });
    client.include_router(router);

    feed(&recorder, "hi").await;
    assert_eq!(take(&log), ["HI"]);
}
