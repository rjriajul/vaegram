//! Dialogues driven through the public test kit, as a bot's own tests would drive them.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use vaegram::dialogue::{Dialogue, DialogueKey, DialogueRouter, DialogueStorage, MemoryStorage, Scope};
use vaegram::filters;
use vaegram::test::{CallKind, Recorder, TestBuilder, TestCallbackQuery, TestChat, TestMessage, TestUser};
use vaegram::{Client, Result};

/// Runs `f` and fails if it takes 5 seconds: a test client answers at once.
async fn soon<T>(f: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), f).await.expect("it hung")
}

#[derive(Clone, Debug, PartialEq)]
enum Signup {
    Name,
    Age { name: String },
    Confirm { name: String, age: u8 },
}

/// Name, then age, then a button press to confirm; `/cancel` at any point.
fn signup(storage: Arc<MemoryStorage<Signup>>) -> DialogueRouter<Signup> {
    let mut flow = DialogueRouter::new("signup", storage);
    flow.on_message(filters::any_state(), filters::command("cancel"), |msg, dlg: Dialogue<Signup>| async move {
        dlg.clear().await?;
        msg.reply("Cancelled.").await
    });
    flow.on_message(filters::no_state(), filters::command("signup"), |msg, dlg| async move {
        dlg.set(Signup::Name).await?;
        msg.reply("Name?").await
    });
    flow.on_message(filters::state_is(Signup::Name), filters::text(), |msg, dlg| async move {
        dlg.set(Signup::Age { name: msg.text().to_owned() }).await?;
        msg.reply("Age?").await
    });
    let asked_age = filters::state(|s: &Signup| matches!(s, Signup::Age { .. }));
    flow.on_message(asked_age, filters::text(), |msg, dlg| async move {
        let Some(Signup::Age { name }) = dlg.state() else { return Ok(()) };
        let Ok(age) = msg.text().trim().parse::<u8>() else {
            msg.reply("A number, please.").await?;
            return Ok(());
        };
        dlg.set(Signup::Confirm { name: name.clone(), age }).await?;
        msg.reply(format!("Save {name}, {age}?")).await?;
        Ok(())
    });
    let confirming = filters::state(|s: &Signup| matches!(s, Signup::Confirm { .. }));
    flow.on_callback_query(confirming, filters::data("yes"), |query, dlg| async move {
        let Some(Signup::Confirm { name, age }) = dlg.state() else { return Ok(()) };
        dlg.clear().await?;
        query.answer().text("Saved").await?;
        let chat = query.chat().map_or(query.from_user.id, |chat| chat.id);
        query.client().send_message(chat, format!("Saved {name}, {age}.")).await?;
        Ok(())
    });
    flow
}

async fn say(recorder: &Recorder, chat: &TestChat, user: i64, text: &str) {
    soon(recorder.feed(TestMessage::in_chat(chat.clone(), TestUser::new(user), text))).await;
}

async fn state_of(flow: &DialogueRouter<Signup>, chat_id: i64, user_id: i64) -> Result<Option<Signup>> {
    Ok(flow.dialogue(flow.key(chat_id, user_id)).await?.state())
}

#[tokio::test]
async fn three_steps_collect_name_and_age_then_a_press_confirms() {
    let (client, recorder) = Client::recording();
    let storage = Arc::new(MemoryStorage::new());
    let flow = signup(Arc::clone(&storage));
    let probe = flow.clone();
    client.include_router(flow.into_router());

    for text in ["/signup", "Ann", "thirty", "30"] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
    }
    assert_eq!(recorder.sent_texts(42), ["Name?", "Age?", "A number, please.", "Save Ann, 30?"]);
    let confirm = Signup::Confirm { name: "Ann".into(), age: 30 };
    assert_eq!(state_of(&probe, 42, 42).await.unwrap(), Some(confirm));

    // A press of another button is no step's: the conversation stays where it is.
    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "no"))).await;
    assert_eq!(storage.len(), 1);

    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "yes"))).await;
    assert_eq!(recorder.sent_texts(42).last().map(String::as_str), Some("Saved Ann, 30."));
    assert_eq!(state_of(&probe, 42, 42).await.unwrap(), None);
    assert!(storage.is_empty());
}

#[tokio::test]
async fn updates_no_step_takes_go_on_to_later_routes() {
    let (client, recorder) = Client::recording();
    client.include_router(signup(Arc::new(MemoryStorage::new())).into_router());
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });

    // No conversation yet: plain text isn't the dialogue's, so the echo gets it.
    soon(recorder.feed(TestMessage::private(42, "hello"))).await;
    // The dialogue takes these, and the echo doesn't see them.
    soon(recorder.feed(TestMessage::private(42, "/signup"))).await;
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    assert_eq!(recorder.sent_texts(42), ["echo", "Name?", "Age?"]);
}

#[tokio::test]
async fn handle_message_runs_the_steps_without_a_router() {
    let (client, recorder) = Client::recording();
    let flow = Arc::new(signup(Arc::new(MemoryStorage::new())));
    let taken = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&taken);
    client.on_message(filters::all(), move |msg| {
        let (flow, log) = (Arc::clone(&flow), Arc::clone(&log));
        async move {
            let took = flow.handle_message(&msg).await?;
            log.lock().unwrap().push(took);
            Ok(())
        }
    });

    for text in ["hi", "/signup", "Ann"] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
    }
    assert_eq!(*taken.lock().unwrap(), [false, true, true]);
    assert_eq!(recorder.sent_texts(42), ["Name?", "Age?"]);
}

#[tokio::test]
async fn two_users_in_one_group_keep_their_own_state() {
    let (client, recorder) = Client::recording();
    let flow = signup(Arc::new(MemoryStorage::new()));
    let probe = flow.clone();
    client.include_router(flow.into_router());
    let group = TestChat::supergroup(77);

    say(&recorder, &group, 1, "/signup").await;
    say(&recorder, &group, 2, "/signup").await;
    say(&recorder, &group, 1, "Ann").await;
    say(&recorder, &group, 2, "Bob").await;
    say(&recorder, &group, 2, "41").await;
    say(&recorder, &group, 1, "30").await;

    let expected = ["Name?", "Name?", "Age?", "Age?", "Save Bob, 41?", "Save Ann, 30?"];
    assert_eq!(recorder.sent_texts(group.id()), expected);
    let ann = Signup::Confirm { name: "Ann".into(), age: 30 };
    let bob = Signup::Confirm { name: "Bob".into(), age: 41 };
    assert_eq!(state_of(&probe, group.id(), 1).await.unwrap(), Some(ann));
    assert_eq!(state_of(&probe, group.id(), 2).await.unwrap(), Some(bob));
}

#[tokio::test]
async fn under_the_topic_scope_each_topic_has_its_own_state() {
    let (client, recorder) = Client::recording();
    let flow = signup(Arc::new(MemoryStorage::new())).scope(Scope::UserInTopic);
    client.include_router(flow.into_router());
    let forum = TestChat::supergroup(77).forum(true);
    let in_topic =
        |topic: i32, text: &str| TestMessage::in_chat(forum.clone(), TestUser::new(1), text).thread_id(topic);

    soon(recorder.feed(in_topic(10, "/signup"))).await;
    // The same user in another topic has no conversation there.
    soon(recorder.feed(in_topic(20, "Ann"))).await;
    soon(recorder.feed(in_topic(10, "Ann"))).await;
    assert_eq!(recorder.sent_texts(forum.id()), ["Name?", "Age?"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_users_quick_messages_run_one_at_a_time() {
    let (client, recorder) = Client::recording();
    let mut counter = DialogueRouter::new("count", MemoryStorage::<u32>::new());
    counter.on_message(filters::no_state(), filters::command("start"), |_msg, dlg: Dialogue<u32>| async move {
        dlg.set(0).await
    });
    counter.on_message(filters::any_state(), filters::text(), |_msg, dlg: Dialogue<u32>| async move {
        let seen = dlg.state().unwrap_or_default();
        // Another tick of the same user would read the same count here, were they not run one at a time.
        tokio::task::yield_now().await;
        dlg.set(seen + 1).await
    });
    let probe = counter.clone();
    client.include_router(counter.into_router());

    soon(recorder.feed(TestMessage::private(42, "/start"))).await;
    for _ in 0..20 {
        recorder.dispatch(TestMessage::private(42, "tick")).await;
    }
    soon(recorder.settle()).await;
    assert_eq!(probe.dialogue(probe.key(42, 42)).await.unwrap().state(), Some(20));
}

#[tokio::test]
async fn a_conversation_ends_when_it_times_out_or_is_cancelled() {
    let (client, recorder) = Client::recording();
    let storage = Arc::new(MemoryStorage::new());
    client.include_router(signup(Arc::clone(&storage)).timeout(Duration::ZERO).into_router());

    soon(recorder.feed(TestMessage::private(42, "/signup"))).await;
    // The state timed out as it was saved: the name finds no conversation, and the expired record is gone.
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    assert_eq!(recorder.sent_texts(42), ["Name?"]);
    assert!(storage.is_empty());

    let (client, recorder) = Client::recording();
    let storage = Arc::new(MemoryStorage::new());
    client.include_router(signup(Arc::clone(&storage)).into_router());
    for text in ["/signup", "Ann", "/cancel", "30"] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
    }
    // After `/cancel` the age is no step's.
    assert_eq!(recorder.sent_texts(42), ["Name?", "Age?", "Cancelled."]);
    assert!(storage.is_empty());
}

#[tokio::test]
async fn the_storage_keeps_the_state_for_the_next_client() {
    let storage = Arc::new(MemoryStorage::new());
    let (first, recorder) = Client::recording();
    first.include_router(signup(Arc::clone(&storage)).timeout(Duration::from_secs(600)).into_router());
    for text in ["/signup", "Ann"] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
    }

    let key = DialogueKey::new("signup", 42, 42);
    let record = DialogueStorage::<Signup>::get(&*storage, &key).await.unwrap().expect("the state is saved");
    assert_eq!(record.state, Signup::Age { name: "Ann".into() });
    assert!(record.expires_at.is_some_and(|at| at > SystemTime::now()));

    // Another client on the same storage, as after a restart, carries the conversation on.
    let (second, recorder) = Client::recording();
    second.include_router(signup(Arc::clone(&storage)).into_router());
    soon(recorder.feed(TestMessage::private(42, "30"))).await;
    assert_eq!(recorder.sent_texts(42), ["Save Ann, 30?"]);
}

#[tokio::test]
async fn a_topic_press_whose_message_cannot_be_read_goes_on_to_later_routes() {
    // A recording client has no answer for `messages.getMessages`, so the pressed message can't be read.
    let (client, recorder) = Client::recording();
    let mut flow = DialogueRouter::new("topic", MemoryStorage::<u8>::new()).scope(Scope::UserInTopic);
    let always = filters::any_state() | filters::no_state();
    flow.on_callback_query(always, filters::all(), |query, _dlg: Dialogue<u8>| async move {
        query.answer().text("dialogue").await
    });
    client.include_router(flow.into_router());
    client.on_callback_query(filters::all(), |query| async move { query.answer().text("later").await });

    soon(recorder.feed(TestCallbackQuery::new(42, -5, 1, "x"))).await;
    let answers: Vec<_> = recorder
        .calls()
        .into_iter()
        .filter_map(|call| match call.kind {
            CallKind::AnswerCallbackQuery { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(answers, [Some("later".to_owned())]);
}

#[tokio::test(start_paused = true)]
async fn a_conversation_waiting_for_its_turn_holds_no_handler_slot() {
    let (client, recorder) = TestBuilder::new().max_concurrent_handlers(2).recording();
    let done = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&done);
    let mut flow = DialogueRouter::new("slow", MemoryStorage::<u8>::new());
    let always = filters::any_state() | filters::no_state();
    flow.on_message(always, filters::text(), move |msg, dlg: Dialogue<u8>| {
        let log = Arc::clone(&log);
        async move {
            if msg.chat.id == 1 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            dlg.set(1).await?;
            log.lock().unwrap().push(msg.chat.id);
            Ok(())
        }
    });
    client.include_router(flow.into_router());

    // Four updates of one conversation, each a second long, then one of another chat.
    for _ in 0..4 {
        recorder.dispatch(TestMessage::private(1, "slow")).await;
    }
    recorder.dispatch(TestMessage::private(2, "quick")).await;
    tokio::time::timeout(Duration::from_secs(60), recorder.settle()).await.expect("it hung");

    let done = done.lock().unwrap().clone();
    assert_eq!(done.len(), 5, "{done:?}");
    // User 1's updates waited for each other, not for a slot: user 2's ran at once, before any of them ended.
    assert_eq!(done[0], 2, "{done:?}");
}
