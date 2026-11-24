//! The update side of `vaegram::test`, used as a bot's own tests would use it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vaegram::enums::ChatType;
use vaegram::filters;
use vaegram::test::{
    CallKind, Recorder, Reply, TestBuilder, TestCallbackQuery, TestChat, TestError, TestMessage, TestUpdate, TestUser,
};
use vaegram::utils::get_channel_id;
use vaegram::{Client, Error, RpcError};

const SECOND: Duration = Duration::from_secs(1);

/// Runs `f` and fails if it takes 5 seconds: a test client answers at once.
async fn soon<T>(f: impl Future<Output = T>) -> T {
    tokio::time::timeout(5 * SECOND, f).await.expect("it hung")
}

/// Waits for the next `messages.sendMessage` the client sends.
async fn next_send(recorder: &Recorder) -> CallKind {
    loop {
        let call = soon(recorder.recv()).await.expect("the client is gone");
        if call.method == "messages.sendMessage" {
            return call.kind;
        }
    }
}

fn sent_text(kind: &CallKind) -> (i64, &str) {
    let CallKind::SendMessage { chat_id, text, .. } = kind else { panic!("not a send: {kind:?}") };
    (*chat_id, text.as_str())
}

#[tokio::test]
async fn command_handler_replies_and_the_reply_is_recorded() {
    let (client, recorder) = Client::recording();
    client.on_message(filters::command("start"), |msg| async move { msg.reply("Welcome!").await });

    soon(recorder.feed(TestMessage::private(42, "/start"))).await;
    assert_eq!(recorder.sent_texts(42), ["Welcome!"]);
    let calls = recorder.calls();
    assert!(
        calls.iter().any(|c| matches!(&c.kind, CallKind::SendMessage { reply_to_message_id: Some(_), .. })),
        "{calls:?}"
    );

    // The fed user is known: sending to it by id needs no lookup.
    client.send_message(42, "again").await.unwrap();
    assert_eq!(recorder.sent_texts(42), ["Welcome!", "again"]);
    assert!(recorder.calls().iter().all(|c| c.method != "users.getUsers"));
}

#[tokio::test]
async fn a_filter_that_does_not_match_records_nothing() {
    let (client, recorder) = Client::recording();
    client.on_message(filters::command("start"), |msg| async move { msg.reply("Welcome!").await });

    soon(recorder.feed(TestMessage::private(42, "/help"))).await;
    soon(recorder.feed(TestMessage::private(42, "start"))).await;
    assert!(recorder.calls().is_empty(), "{:?}", recorder.calls());

    // The same handler does answer a matching update fed the same way.
    soon(recorder.feed(TestMessage::private(42, "/start"))).await;
    assert_eq!(recorder.sent_texts(42), ["Welcome!"]);
}

#[tokio::test]
async fn callback_query_answer_is_recorded() {
    let (client, recorder) = Client::recording();
    client.on_callback_query(filters::all(), |query| async move { query.answer().text("Clicked").alert().await });

    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "btn:ok").query_id(99))).await;
    let calls = recorder.calls();
    assert!(
        calls.iter().any(|c| matches!(
            &c.kind,
            CallKind::AnswerCallbackQuery { query_id: 99, text: Some(t), alert: true, .. } if t == "Clicked"
        )),
        "{calls:?}"
    );
}

#[tokio::test]
async fn callback_query_in_a_channel_names_the_channel() {
    let (client, recorder) = Client::recording();
    let kinds = Arc::new(Mutex::new(Vec::new()));
    let seen = kinds.clone();
    client.on_callback_query(filters::all(), move |query| {
        let seen = seen.clone();
        async move {
            seen.lock().unwrap().push(query.chat().map(|c| (c.id, matches!(c.kind, ChatType::Channel))));
            Ok(())
        }
    });

    let query = TestCallbackQuery::new(42, get_channel_id(5), 1, "x").chat(TestChat::channel(5));
    soon(recorder.feed(query)).await;
    assert_eq!(*kinds.lock().unwrap(), [Some((get_channel_id(5), true))]);
}

#[tokio::test]
async fn group_supergroup_and_channel_messages_reach_handlers() {
    let (client, recorder) = Client::recording();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    client.on_message(filters::all(), move |msg| {
        let log = log.clone();
        async move {
            let kind = match msg.chat.kind {
                ChatType::Group => "group",
                ChatType::Supergroup => "supergroup",
                ChatType::Channel => "channel",
                _ => "other",
            };
            log.lock().unwrap().push((msg.chat.id, kind, msg.from_user.as_ref().map(|u| u.id)));
            Ok(())
        }
    });

    let user = TestUser::new(42).first_name("Ann");
    soon(recorder.feed(TestMessage::in_chat(TestChat::group(7), user.clone(), "hi"))).await;
    soon(recorder.feed(TestMessage::in_chat(TestChat::supergroup(8).title("Super"), user.clone(), "hi"))).await;
    soon(recorder.feed(TestMessage::in_chat(TestChat::channel(9), user, "post"))).await;
    assert_eq!(
        *seen.lock().unwrap(),
        [
            (-7, "group", Some(42)),
            (get_channel_id(8), "supergroup", Some(42)),
            // A channel post comes from the channel, not from a user.
            (get_channel_id(9), "channel", None),
        ]
    );

    // The supergroup and channel are known now: sending there needs no lookup.
    client.send_message(get_channel_id(8), "back").await.unwrap();
    assert!(recorder.calls().iter().all(|c| c.method != "channels.getChannels"));
}

#[tokio::test]
async fn edited_message_reaches_on_edited_message() {
    let (client, recorder) = Client::recording();
    let edits = Arc::new(Mutex::new(Vec::new()));
    let seen = edits.clone();
    client.on_message(filters::all(), |msg| async move { msg.reply("new").await });
    client.on_edited_message(filters::all(), move |msg| {
        let seen = seen.clone();
        async move {
            seen.lock().unwrap().push((msg.id, msg.text().to_owned(), msg.edit_date.is_some()));
            Ok(())
        }
    });

    soon(recorder.feed(TestUpdate::EditedMessage(TestMessage::private(42, "fixed").id(3)))).await;
    assert_eq!(*edits.lock().unwrap(), [(3, "fixed".to_owned(), true)]);
    assert!(recorder.calls().is_empty(), "an edit isn't a new message");
}

#[tokio::test]
async fn reply_and_thread_headers_are_read() {
    let (client, recorder) = Client::recording();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    client.on_message(filters::all(), move |msg| {
        let log = log.clone();
        async move {
            log.lock().unwrap().push((msg.reply_to_message_id, msg.message_thread_id, msg.is_topic_message));
            Ok(())
        }
    });

    let forum = TestChat::supergroup(8).forum(true);
    let user = TestUser::new(42);
    soon(recorder.feed(TestMessage::private(42, "re").reply_to(7))).await;
    soon(recorder.feed(TestMessage::in_chat(forum.clone(), user.clone(), "in topic").thread_id(3))).await;
    soon(recorder.feed(TestMessage::in_chat(forum, user, "re in topic").thread_id(3).reply_to(5))).await;
    assert_eq!(*seen.lock().unwrap(), [(Some(7), None, false), (None, Some(3), true), (Some(5), Some(3), true)]);
}

#[tokio::test]
async fn know_user_and_know_chat_let_a_strict_client_send() {
    let (client, script) = Client::scripted();
    script.know_user(TestUser::new(42)).know_chat(TestChat::supergroup(8));
    script.reply_next("messages.sendMessage", Reply::SentMessage { id: 1 });
    script.reply_next("messages.sendMessage", Reply::SentMessage { id: 2 });

    // Without them the client would ask users.getUsers and channels.getChannels, which this script doesn't answer.
    client.send_message(42, "hi").await.unwrap();
    client.send_message(get_channel_id(8), "hi").await.unwrap();
    let methods: Vec<_> = script.calls().iter().map(|c| c.method).collect();
    assert_eq!(methods, ["messages.sendMessage", "messages.sendMessage"]);

    assert!(client.send_message(43, "hi").await.is_err(), "an unknown user is looked up, and that's unscripted");
}

#[tokio::test]
async fn ask_dialog_takes_two_turns() {
    let (client, recorder) = Client::recording();
    client.on_message(filters::command("name"), |msg| async move {
        let answer = msg.client().ask(msg.chat.id, "What's your name?").await?;
        msg.reply(format!("Hi, {}", answer.text())).await
    });

    // `feed` would wait for the handler, which waits for the answer: start it, then answer.
    soon(recorder.dispatch(TestMessage::private(42, "/name"))).await;
    assert_eq!(sent_text(&next_send(&recorder).await), (42, "What's your name?"));
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    soon(recorder.settle()).await;
    assert_eq!(recorder.sent_texts(42), ["What's your name?", "Hi, Ann"]);
}

#[tokio::test]
async fn feeds_from_two_tasks_do_not_wait_on_each_other() {
    let (client, recorder) = Client::recording();
    client.on_message(filters::command("name"), |msg| async move {
        let answer = msg.client().ask(msg.chat.id, "What's your name?").await?;
        msg.reply(format!("Hi, {}", answer.text())).await
    });

    // The first feed waits for its handler, parked in `ask`, while the answer is fed from here.
    let first = recorder.clone();
    let first = tokio::spawn(async move { first.feed(TestMessage::private(42, "/name")).await });
    next_send(&recorder).await;
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    soon(first).await.unwrap();
    assert_eq!(recorder.sent_texts(42), ["What's your name?", "Hi, Ann"]);
}

#[tokio::test]
async fn ordered_keeps_one_chats_handlers_in_order() {
    let (client, recorder) = TestBuilder::new().ordered(true).recording();
    let log = Arc::new(Mutex::new(Vec::new()));
    let seen = log.clone();
    client.on_message(filters::all(), move |msg| {
        let seen = seen.clone();
        async move {
            if msg.text() == "slow" {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            seen.lock().unwrap().push(msg.text().to_owned());
            Ok(())
        }
    });

    soon(recorder.dispatch(TestMessage::private(42, "slow"))).await;
    soon(recorder.dispatch(TestMessage::private(42, "fast"))).await;
    soon(recorder.settle()).await;
    assert_eq!(*log.lock().unwrap(), ["slow", "fast"]);
}

#[tokio::test]
async fn max_concurrent_handlers_caps_the_handlers_running() {
    let (client, recorder) = TestBuilder::new().max_concurrent_handlers(1).recording();
    let (now, most) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (running, peak) = (now.clone(), most.clone());
    client.on_message(filters::all(), move |_| {
        let (running, peak) = (running.clone(), peak.clone());
        async move {
            peak.fetch_max(running.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(20)).await;
            running.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        }
    });

    // Three chats, so `ordered` isn't what keeps them apart.
    for chat in [1, 2, 3] {
        soon(recorder.dispatch(TestMessage::private(chat, "hi"))).await;
    }
    soon(recorder.settle()).await;
    assert_eq!(most.load(Ordering::SeqCst), 1);
    assert_eq!(now.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn on_error_sees_a_scripted_rpc_error() {
    let (client, script) = Client::scripted();
    script.auto_reply(true);
    let errors = Arc::new(Mutex::new(Vec::new()));
    let seen = errors.clone();
    client.on_message(filters::command("ping"), |msg| async move { msg.reply("pong").await });
    client.on_error(move |e| {
        seen.lock().unwrap().push(e.error.is("BOT_WAS_BLOCKED"));
        async {}
    });

    script.fail_next("messages.sendMessage", RpcError::new(403, "BOT_WAS_BLOCKED", None));
    soon(script.feed(TestMessage::private(42, "/ping"))).await;
    assert_eq!(*errors.lock().unwrap(), [true]);
}

#[tokio::test]
async fn run_takes_pushed_updates_until_the_stream_closes() {
    let (client, recorder) = Client::recording();
    client.on_message(filters::command("ping"), |msg| async move { msg.reply("pong").await });
    let running = client.clone();
    let run = tokio::spawn(async move { running.run().await });

    // Twice: the reply's `pts` reaches `run`, so the second push follows it with no gap to wait out.
    for _ in 0..2 {
        soon(recorder.push_stream(TestMessage::private(42, "/ping"))).await.unwrap();
        assert_eq!(sent_text(&next_send(&recorder).await), (42, "pong"));
    }

    recorder.close_stream();
    let result = soon(run).await.unwrap();
    assert!(matches!(result, Err(Error::Disconnected)), "{result:?}");
    let err = soon(recorder.push_stream(TestMessage::private(42, "/ping"))).await.unwrap_err();
    assert_eq!(err.downcast_ref::<TestError>(), Some(&TestError::NoRun));
}

#[tokio::test]
async fn script_push_stream_reaches_run() {
    let (client, script) = Client::scripted();
    script.auto_reply(true);
    client.on_message(filters::command("ping"), |msg| async move { msg.reply("pong").await });
    let running = client.clone();
    let run = tokio::spawn(async move { running.run().await });

    soon(script.push_stream(TestMessage::private(42, "/ping"))).await.unwrap();
    assert_eq!(sent_text(&next_send(script.recorder()).await), (42, "pong"));
    client.stop();
    assert!(soon(run).await.unwrap().is_ok());
}

#[tokio::test]
async fn push_stream_fails_when_run_cannot_start() {
    // Strict: nothing answers getMe, so `run` fails before it takes updates.
    let (client, script) = Client::scripted();
    let running = client.clone();
    let run = tokio::spawn(async move { running.run().await });

    let err = soon(script.push_stream(TestMessage::private(1, "x"))).await.unwrap_err();
    assert_eq!(err.downcast_ref::<TestError>(), Some(&TestError::NoRun));
    assert!(soon(run).await.unwrap().is_err());
}
