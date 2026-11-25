//! The request side of `vaegram::test`, used as a bot's own tests would use it.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use vaegram::enums::{ClientEvent, ParseMode};
use vaegram::raw::functions;
use vaegram::session::AutoSleep;
use vaegram::test::{CallKind, RecordedCall, Reply, TestBuilder, TestError};
use vaegram::types::{Media, Upload};
use vaegram::{Client, Error, RpcError};

const SECOND: Duration = Duration::from_secs(1);

fn sends(calls: &[RecordedCall]) -> Vec<&CallKind> {
    calls.iter().filter(|c| c.method == "messages.sendMessage").map(|c| &c.kind).collect()
}

fn test_error(error: &Error) -> Option<&TestError> {
    error.downcast_ref::<TestError>()
}

/// Runs `f` and fails if it takes 5 seconds: what a test client answers comes back at once.
async fn soon<T>(f: impl Future<Output = T>) -> T {
    tokio::time::timeout(5 * SECOND, f).await.expect("it waited, as a dial or a sleep would")
}

#[tokio::test]
async fn send_message_is_recorded_and_returns_the_message() {
    let (client, recorder) = Client::recording();
    let sent = client.send_message(42, "hi").await.unwrap();
    assert_eq!((sent.id, sent.chat.id, sent.text(), sent.outgoing), (1, 42, "hi", true));

    let calls = recorder.calls();
    let [kind] = sends(&calls)[..] else { panic!("{calls:?}") };
    let CallKind::SendMessage { chat_id, text, reply_to_message_id, .. } = kind else { panic!("{kind:?}") };
    assert_eq!((*chat_id, text.as_str(), *reply_to_message_id), (42, "hi", None));
    assert_eq!(recorder.sent_texts(42), ["hi"]);
    assert!(recorder.sent_texts(43).is_empty());

    let next = client.send_message(42, "again").await.unwrap();
    assert_eq!(next.id, 2, "message ids go up by one");
}

#[tokio::test]
async fn a_reply_names_the_message_it_replies_to() {
    let (client, recorder) = Client::recording();
    let first = client.send_message(42, "question").await.unwrap();
    let reply = first.reply("answer").await.unwrap();
    assert_eq!(reply.reply_to_message_id, Some(first.id));

    let calls = recorder.calls();
    let last = sends(&calls).pop().unwrap();
    let CallKind::SendMessage { chat_id, text, reply_to_message_id, .. } = last else { panic!("{last:?}") };
    assert_eq!((*chat_id, text.as_str(), *reply_to_message_id), (42, "answer", Some(first.id)));
}

#[tokio::test]
async fn a_scripted_flood_wait_comes_back_at_once() {
    let (client, script) = Client::scripted();
    script.auto_reply(true);
    script.fail_next("messages.sendMessage", RpcError::new(420, "FLOOD_WAIT", Some(30)));

    let error = soon(async { client.send_message(42, "hi").await }).await.unwrap_err();
    assert_eq!(error.flood_wait(), Some(30 * SECOND));
    // Only the next request failed.
    client.send_message(42, "hi").await.unwrap();
    assert_eq!(sends(&script.calls()).len(), 2);
}

#[tokio::test(start_paused = true)]
async fn auto_sleep_sleeps_a_flood_wait_off() {
    let (client, script) = TestBuilder::new().auto_sleep(AutoSleep::new()).scripted();
    script.auto_reply(true);
    script.fail_next("messages.sendMessage", RpcError::new(420, "FLOOD_WAIT", Some(5)));
    let start = tokio::time::Instant::now();
    client.send_message(42, "hi").await.unwrap();
    assert!(start.elapsed() >= 5 * SECOND, "slept {:?}", start.elapsed());
    assert_eq!(sends(&script.calls()).len(), 2, "sent again after the wait");
}

#[tokio::test]
async fn an_unscripted_request_fails_with_its_name() {
    let (client, script) = Client::scripted();
    let error = client.answer_callback_query(7).await.unwrap_err();
    assert_eq!(test_error(&error), Some(&TestError::Unscripted { method: "messages.setBotCallbackAnswer" }));
    assert!(error.to_string().contains("unscripted request messages.setBotCallbackAnswer"), "{error}");
    assert_eq!(script.calls().len(), 1, "the failed request is still recorded");
}

#[tokio::test]
async fn a_request_without_a_default_answer_says_so() {
    let (client, recorder) = Client::recording();
    let error = client.get_messages(42, &[1]).await.unwrap_err();
    assert!(matches!(test_error(&error), Some(TestError::NoDefaultAnswer { .. })), "{error}");
    assert!(!recorder.calls().is_empty());
}

#[tokio::test]
async fn on_call_answers_every_request_of_its_method() {
    let (client, script) = Client::scripted();
    let seen = Arc::new(AtomicUsize::new(0));
    let count = seen.clone();
    script.on_call("messages.setBotCallbackAnswer", move |call| {
        count.fetch_add(1, Ordering::SeqCst);
        assert!(matches!(call.kind, CallKind::AnswerCallbackQuery { query_id: 99, .. }));
        Reply::Bool(true)
    });
    client.answer_callback_query(99).text("Clicked").alert().await.unwrap();
    client.answer_callback_query(99).await.unwrap();
    assert_eq!(seen.load(Ordering::SeqCst), 2);

    let kinds: Vec<CallKind> = script.calls().into_iter().map(|c| c.kind).collect();
    let [first, second] = &kinds[..] else { panic!("{kinds:?}") };
    let CallKind::AnswerCallbackQuery { query_id, text, alert, .. } = first else { panic!("{first:?}") };
    assert_eq!((*query_id, text.as_deref(), *alert), (99, Some("Clicked"), true));
    let CallKind::AnswerCallbackQuery { query_id, text, alert, .. } = second else { panic!("{second:?}") };
    assert_eq!((*query_id, text.as_deref(), *alert), (99, None, false));

    // A queued reply goes before the responder.
    script.fail_next("messages.setBotCallbackAnswer", RpcError::new(400, "QUERY_ID_INVALID", None));
    assert!(client.answer_callback_query(99).await.unwrap_err().is("QUERY_ID_INVALID"));
    client.answer_callback_query(99).await.unwrap();
}

#[tokio::test]
async fn a_panicking_responder_fails_its_request_and_the_client_goes_on() {
    let (client, script) = Client::scripted();
    script.on_call("messages.setBotCallbackAnswer", |_| panic!("boom"));
    let error = soon(async { client.answer_callback_query(1).await }).await.unwrap_err();
    let panicked = TestError::ResponderPanicked { method: "messages.setBotCallbackAnswer", message: "boom".into() };
    assert_eq!(test_error(&error), Some(&panicked));

    script.on_call("messages.setBotCallbackAnswer", |_| Reply::Bool(true));
    soon(async { client.answer_callback_query(1).await }).await.unwrap();
}

#[tokio::test]
async fn reply_next_picks_the_message_id() {
    let (client, script) = Client::scripted();
    script.auto_reply(true).reply_next("messages.sendMessage", Reply::SentMessage { id: 77 });
    assert_eq!(client.send_message(42, "hi").await.unwrap().id, 77);
    assert_eq!(client.send_message(42, "hi").await.unwrap().id, 78, "ids go on after a scripted one");
}

#[tokio::test]
async fn take_calls_drains_and_recv_sees_each_call_once() {
    let (client, recorder) = TestBuilder::new().bot(5, Some("my_bot")).recording();
    client.send_message(42, "one").await.unwrap();
    client.send_message(42, "two").await.unwrap();

    let taken = recorder.take_calls();
    assert_eq!(sends(&taken).len(), 2);
    assert!(recorder.calls().is_empty());
    assert!(recorder.take_calls().is_empty());

    // `recv` keeps its own place: the first request ever sent, the peer lookup, comes first.
    assert_eq!(recorder.recv().await.unwrap().method, "users.getUsers");
    assert_eq!(recorder.recv().await.unwrap().method, "messages.sendMessage");
    assert_eq!(recorder.recv().await.unwrap().method, "messages.sendMessage");
    // Then nothing, once the client is gone.
    drop(client);
    assert!(soon(recorder.recv()).await.is_none());
}

#[tokio::test]
async fn the_test_account_is_the_one_the_builder_names() {
    let (client, _recorder) = TestBuilder::new().bot(5, Some("my_bot")).recording();
    let me = client.get_me().await.unwrap();
    assert_eq!((me.id, me.is_bot, me.username.as_deref()), (5, true, Some("my_bot")));

    let (client, _recorder) = TestBuilder::new().user(9, true).recording();
    let me = client.get_me().await.unwrap();
    assert_eq!((me.id, me.is_bot), (9, false));
}

#[tokio::test]
async fn the_parse_mode_applies_before_the_text_is_recorded() {
    let (client, recorder) = TestBuilder::new().parse_mode(ParseMode::Html).recording();
    client.send_message(42, "<b>x</b>").await.unwrap();
    assert_eq!(recorder.sent_texts(42), ["x"]);
}

#[tokio::test]
async fn edits_and_deletes_are_answered_in_private_chats_and_supergroups() {
    let (client, recorder) = Client::recording();
    let edited = client.edit_message_text(42, 3, "new").await.unwrap();
    assert_eq!((edited.id, edited.text()), (3, "new"));
    client.delete_messages(42, &[3]).await.unwrap();

    let group = -1001234567890;
    let sent = client.send_message(group, "hi").await.unwrap();
    assert_eq!(sent.chat.id, group);
    let edited = client.edit_message_text(group, sent.id, "edited").await.unwrap();
    assert_eq!((edited.chat.id, edited.text()), (group, "edited"));
    client.delete_messages(group, &[sent.id]).await.unwrap();

    let kinds: Vec<CallKind> = recorder.calls().into_iter().map(|c| c.kind).collect();
    let edit = |chat| kinds.iter().any(|k| matches!(k, CallKind::EditMessage { chat_id, .. } if *chat_id == chat));
    assert!(edit(42) && edit(group), "{kinds:?}");
    let private = kinds.iter().any(|k| matches!(k, CallKind::DeleteMessages { chat_id: None, .. }));
    let channel = kinds.iter().any(|k| matches!(k, CallKind::DeleteMessages { chat_id: Some(c), .. } if *c == group));
    assert!(private && channel, "{kinds:?}");
}

#[tokio::test]
async fn a_sent_photo_has_its_photo() {
    let (client, recorder) = Client::recording();
    let sent = soon(async { client.send_photo(42, Upload::bytes("a.jpg", b"x")).await }).await.unwrap();
    assert!(matches!(sent.media, Some(Media::Photo(_))), "{:?}", sent.media);
    assert!(recorder.calls().iter().any(|c| matches!(c.kind, CallKind::SendMedia { chat_id: 42, .. })));
}

#[tokio::test]
async fn a_migration_is_refused_without_dialing() {
    let (client, script) = Client::scripted();
    script.fail_next("auth.importBotAuthorization", RpcError::new(303, "USER_MIGRATE", Some(4)));
    let error = soon(client.sign_in_bot("1:token")).await.unwrap_err();
    assert_eq!(test_error(&error), Some(&TestError::NoConnection { dc_id: 4 }));
}

#[tokio::test]
async fn an_offline_client_sends_nothing() {
    let client = Client::offline();
    let error = soon(async { client.send_message(42, "hi").await }).await.unwrap_err();
    assert!(matches!(error, Error::Disconnected), "{error}");
    let error = soon(async { client.upload_file(Upload::bytes("a.txt", b"hi")).await }).await.unwrap_err();
    assert!(matches!(error, Error::Disconnected), "{error}");
    let error = soon(client.invoke_in_dc(4, &functions::help::GetConfig {})).await.unwrap_err();
    assert!(matches!(error, Error::Disconnected), "{error}");
}

#[tokio::test]
async fn file_transfers_stay_in_the_process() {
    // A user account: its parts go unpaced, so the upload takes no time.
    let (client, recorder) = TestBuilder::new().user(9, false).recording();
    let failed = Arc::new(AtomicUsize::new(0));
    let count = failed.clone();
    client.on_event(move |event| {
        if matches!(event, ClientEvent::ConnectFailed { .. }) {
            count.fetch_add(1, Ordering::SeqCst);
        }
    });

    // Big enough to go as a big file over several connections; it gets the one it has.
    let big = vec![7u8; 12 << 20];
    let file = soon(async { client.upload_file(Upload::bytes("big.bin", &big)).await }).await.unwrap();
    assert_eq!(file.name(), "big.bin");
    assert!(recorder.calls().iter().any(|c| c.method == "upload.saveBigFilePart"));
    assert_eq!(failed.load(Ordering::SeqCst), 0, "no connection was tried");

    // Another data center would need a connection of its own: refused before any dial.
    let error = soon(client.invoke_in_dc(4, &functions::help::GetConfig {})).await.unwrap_err();
    assert_eq!(test_error(&error), Some(&TestError::NoConnection { dc_id: 4 }));
}

#[cfg(feature = "json")]
#[tokio::test]
async fn a_call_reads_as_json() {
    let (client, recorder) = Client::recording();
    client.answer_callback_query(5).await.unwrap();
    let json = recorder.calls()[0].request_json();
    assert!(json.contains("messages.setBotCallbackAnswer"), "{json}");
}

#[cfg(feature = "unstable-raw")]
#[tokio::test]
async fn raw_replies_go_first_whatever_the_method() {
    use vaegram::raw::Serializable;
    let (client, script) = Client::scripted();
    script.push_raw(Ok(true.to_bytes()));
    client.answer_callback_query(5).await.unwrap();
    assert!(!script.calls()[0].body().is_empty());
}
