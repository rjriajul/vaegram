//! Conversations driven through the public test kit, as a bot's own tests would drive them.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use vaegram::Client;
use vaegram::conversation::{Answer, Conversation, Scope};
use vaegram::enums::ClientEvent;
use vaegram::filters;
use vaegram::test::{CallKind, Recorder, TestBuilder, TestCallbackQuery, TestChat, TestMessage, TestUser};
use vaegram::types::{CallbackQuery, Message};

/// Runs `f` and fails if it takes 5 seconds: a test client answers at once.
async fn soon<T>(f: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), f).await.expect("it hung")
}

/// Waits until `done` holds. A conversation runs on a task of its own, and what it passes on reaches the handlers
/// once the dispatcher routes it again, so this settles and looks again until it holds.
async fn until(recorder: &Recorder, done: impl Fn() -> bool) -> bool {
    let reached = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            recorder.settle().await;
            if done() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await;
    reached.is_ok()
}

/// Waits until the client has sent exactly `expected` to `chat`.
async fn texts(recorder: &Recorder, chat: i64, expected: &[&str]) {
    let reached = until(recorder, || recorder.sent_texts(chat) == expected).await;
    assert!(reached, "sent {:?}, expected {expected:?}", recorder.sent_texts(chat));
}

/// The texts of the button press answers the client sent.
fn answers(recorder: &Recorder) -> Vec<String> {
    let answer = |call: vaegram::test::RecordedCall| match call.kind {
        CallKind::AnswerCallbackQuery { text, .. } => Some(text.unwrap_or_default()),
        _ => None,
    };
    recorder.calls().into_iter().filter_map(answer).collect()
}

/// Waits until the client has answered exactly `expected` to button presses.
async fn answered(recorder: &Recorder, expected: &[&str]) {
    let reached = until(recorder, || answers(recorder) == expected).await;
    assert!(reached, "answered {:?}, expected {expected:?}", answers(recorder));
}

/// The errors `on_error` gets, as text.
fn errors(client: &Client) -> Arc<Mutex<Vec<String>>> {
    let log = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&log);
    client.on_error(move |e| {
        let sink = Arc::clone(&sink);
        async move { sink.lock().unwrap().push(e.error.to_string()) }
    });
    log
}

/// A name, an age checked as a number, then a press of "yes"; `/signup` starts it, `/cancel` ends it, and any other
/// text nobody wants is echoed.
fn signup(client: &Client) {
    client
        .conversation("signup", |mut conv: Conversation, msg: Message| async move {
            msg.reply("Name?").await?;
            let name = conv.wait_for(filters::text()).await?.text().to_owned();
            msg.reply("Age?").await?;
            let age: u8 =
                conv.form().number().otherwise(|m: Message| async move { m.reply("A number, please.").await }).await?;
            msg.reply(format!("Save {name}, {age}?")).await?;
            let press = conv.wait_for_callback_query(filters::data("yes")).await?;
            press.answer().await?;
            msg.reply(format!("Saved {name}, {age}.")).await?;
            Ok(())
        })
        .ignore(filters::command(["signup", "cancel"]));
    client.on_message(filters::command("signup"), |msg| async move {
        msg.client().enter_conversation("signup", msg.clone())
    });
    client.on_message(filters::command("cancel"), |msg| async move {
        let ended = msg.client().exit_conversation("signup", &msg);
        msg.reply(format!("Cancelled {ended}.")).await
    });
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });
}

async fn say(recorder: &Recorder, chat: &TestChat, user: i64, text: &str) {
    soon(recorder.feed(TestMessage::in_chat(chat.clone(), TestUser::new(user), text))).await;
}

#[tokio::test]
async fn three_steps_ask_check_the_number_and_take_a_press() {
    let (client, recorder) = Client::recording();
    let errors = errors(&client);
    signup(&client);

    soon(recorder.feed(TestMessage::private(42, "/signup"))).await;
    texts(&recorder, 42, &["Name?"]).await;
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    texts(&recorder, 42, &["Name?", "Age?"]).await;
    // Not a number: `otherwise` asks again and the conversation keeps the message.
    soon(recorder.feed(TestMessage::private(42, "thirty"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "A number, please."]).await;
    soon(recorder.feed(TestMessage::private(42, "30"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "A number, please.", "Save Ann, 30?"]).await;
    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "yes"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "A number, please.", "Save Ann, 30?", "Saved Ann, 30."]).await;

    // The conversation is over: the next text is the echo's.
    soon(recorder.feed(TestMessage::private(42, "hello"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "A number, please.", "Save Ann, 30?", "Saved Ann, 30.", "echo"]).await;
    assert!(errors.lock().unwrap().is_empty(), "{errors:?}");
}

#[tokio::test]
async fn two_users_in_one_group_run_their_own_conversations() {
    let (client, recorder) = Client::recording();
    signup(&client);
    let group = TestChat::supergroup(77);
    let steps = [
        (1, "/signup", "Name?"),
        (2, "/signup", "Name?"),
        (1, "Ann", "Age?"),
        (2, "Bob", "Age?"),
        (2, "41", "Save Bob, 41?"),
        (1, "30", "Save Ann, 30?"),
    ];
    let mut expected = Vec::new();
    for (user, text, reply) in steps {
        say(&recorder, &group, user, text).await;
        expected.push(reply);
        texts(&recorder, group.id(), &expected).await;
    }
    for (user, reply) in [(2, "Saved Bob, 41."), (1, "Saved Ann, 30.")] {
        soon(recorder.feed(TestCallbackQuery::new(user, group.id(), 1, "yes").chat(group.clone()))).await;
        expected.push(reply);
        texts(&recorder, group.id(), &expected).await;
    }
}

#[tokio::test(start_paused = true)]
async fn a_wait_that_times_out_ends_the_conversation_quietly() {
    let (client, recorder) = Client::recording();
    let errors = errors(&client);
    client
        .conversation("quiz", |mut conv: Conversation, msg: Message| async move {
            msg.reply("2 + 2?").await?;
            let answer = conv.wait_for(filters::text()).await?;
            msg.reply(format!("You said {}.", answer.text())).await?;
            Ok(())
        })
        .timeout(Duration::from_secs(10));
    client.on_message(
        filters::command("quiz"),
        |msg| async move { msg.client().enter_conversation("quiz", msg.clone()) },
    );
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });

    soon(recorder.feed(TestMessage::private(42, "/quiz"))).await;
    texts(&recorder, 42, &["2 + 2?"]).await;
    tokio::time::sleep(Duration::from_secs(11)).await;
    // The wait timed out and its `?` ended the conversation: the late answer is the echo's, and nothing was reported.
    soon(recorder.feed(TestMessage::private(42, "4"))).await;
    texts(&recorder, 42, &["2 + 2?", "echo"]).await;
    assert!(errors.lock().unwrap().is_empty(), "{errors:?}");
}

#[tokio::test]
async fn halt_ends_the_conversation_without_a_report() {
    let (client, recorder) = Client::recording();
    let errors = errors(&client);
    client.conversation("parrot", |mut conv: Conversation, msg: Message| async move {
        msg.reply("Say something.").await?;
        loop {
            let said = conv.wait_for(filters::text()).await?;
            if said.text() == "stop" {
                said.reply("Stopped.").await?;
                return conv.halt();
            }
            said.reply(format!("You said {}.", said.text())).await?;
        }
    });
    client.on_message(filters::command("parrot"), |msg| async move {
        msg.client().enter_conversation("parrot", msg.clone())
    });
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });

    let mut expected = Vec::new();
    for (text, reply) in
        [("/parrot", "Say something."), ("hi", "You said hi."), ("stop", "Stopped."), ("hello", "echo")]
    {
        soon(recorder.feed(TestMessage::private(42, text))).await;
        expected.push(reply);
        texts(&recorder, 42, &expected).await;
    }
    assert!(errors.lock().unwrap().is_empty(), "{errors:?}");
}

#[tokio::test]
async fn updates_the_conversation_skips_or_does_not_want_reach_the_handlers() {
    let (client, recorder) = Client::recording();
    client.conversation("confirm", |mut conv: Conversation, msg: Message| async move {
        msg.reply("Your name?").await?;
        let name = loop {
            let said = conv.wait().await?;
            if said.text().starts_with('/') {
                // A command isn't a name: the handlers get it, as if the conversation weren't there.
                conv.skip(said);
            } else {
                break said.text().to_owned();
            }
        };
        msg.reply(format!("Send /yes to save {name}.")).await?;
        conv.wait_for_command("yes").await?;
        msg.reply("Saved.").await?;
        Ok(())
    });
    client.on_message(filters::command("start"), |msg| async move {
        msg.client().enter_conversation("confirm", msg.clone())
    });
    client.on_message(filters::command("help"), |msg| async move { msg.reply("Help.").await });
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });

    let mut expected = Vec::new();
    for (text, reply) in [
        ("/start", "Your name?"),
        ("/help", "Help."),
        ("Ann", "Send /yes to save Ann."),
        ("hello", "echo"),
        ("/yes", "Saved."),
    ] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
        expected.push(reply);
        texts(&recorder, 42, &expected).await;
    }
}

#[tokio::test]
async fn form_fields_ask_again_or_drop_what_they_cannot_read() {
    let (client, recorder) = Client::recording();
    client.conversation("plan", |mut conv: Conversation, msg: Message| async move {
        msg.reply("Seats?").await?;
        let seats: u8 =
            conv.form().number().otherwise(|m: Message| async move { m.reply("1 to 255, please.").await }).await?;
        msg.reply("Plan?").await?;
        let plan = conv
            .form()
            .select(["free", "pro"])
            .otherwise(|m: Message| async move { m.reply("free or pro?").await })
            .await?;
        msg.reply("Teams?").await?;
        // No `otherwise`: what doesn't parse is dropped, and reaches no handler.
        let teams: u32 = conv.form().number().await?;
        msg.reply(format!("{seats} seats on {plan} for {teams} teams.")).await?;
        Ok(())
    });
    client.on_message(
        filters::command("plan"),
        |msg| async move { msg.client().enter_conversation("plan", msg.clone()) },
    );
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });

    let steps = [
        ("/plan", Some("Seats?")),
        ("300", Some("1 to 255, please.")),
        ("-1", Some("1 to 255, please.")),
        (" 12 ", Some("Plan?")),
        ("gold", Some("free or pro?")),
        ("pro", Some("Teams?")),
        ("many", None),
        ("3", Some("12 seats on pro for 3 teams.")),
    ];
    let mut expected = Vec::new();
    for (text, reply) in steps {
        soon(recorder.feed(TestMessage::private(42, text))).await;
        expected.extend(reply);
        texts(&recorder, 42, &expected).await;
    }
}

#[tokio::test]
async fn cancel_ends_a_conversation_and_a_second_start_is_refused_meanwhile() {
    let (client, recorder) = Client::recording();
    let errors = errors(&client);
    signup(&client);

    soon(recorder.feed(TestMessage::private(42, "/signup"))).await;
    texts(&recorder, 42, &["Name?"]).await;
    // The conversation ignores `/signup`, so the handler gets it, and can't start a second one.
    soon(recorder.feed(TestMessage::private(42, "/signup"))).await;
    let refused = errors.lock().unwrap().clone();
    assert!(refused.len() == 1 && refused[0].contains("already running"), "{refused:?}");

    // `stop_listening` leaves the conversation alone: it stops none and the name still reaches it.
    assert_eq!(soon(client.stop_listening::<Message>().into_future()).await.unwrap(), 0);
    assert_eq!(soon(client.stop_listening::<CallbackQuery>().into_future()).await.unwrap(), 0);
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    texts(&recorder, 42, &["Name?", "Age?"]).await;

    soon(recorder.feed(TestMessage::private(42, "/cancel"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "Cancelled 1."]).await;
    soon(recorder.feed(TestMessage::private(42, "Ann"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "Cancelled 1.", "echo"]).await;
    // Nothing left to end.
    soon(recorder.feed(TestMessage::private(42, "/cancel"))).await;
    texts(&recorder, 42, &["Name?", "Age?", "Cancelled 1.", "echo", "Cancelled 0."]).await;
}

#[tokio::test]
async fn a_waiting_conversation_holds_no_handler_slot() {
    let (client, recorder) = TestBuilder::new().max_concurrent_handlers(1).ordered(true).recording();
    // Registered first, so the echo `signup` adds doesn't take `/ping`.
    client.on_message(filters::command("ping"), |msg| async move { msg.reply("pong").await });
    signup(&client);

    soon(recorder.feed(TestMessage::private(1, "/signup"))).await;
    texts(&recorder, 1, &["Name?"]).await;
    // User 1's conversation waits for a name on its own task: user 2's handler gets the only slot at once.
    soon(recorder.feed(TestMessage::private(2, "/ping"))).await;
    assert_eq!(recorder.sent_texts(2), ["pong"]);
    soon(recorder.feed(TestMessage::private(1, "Ann"))).await;
    texts(&recorder, 1, &["Name?", "Age?"]).await;
}

/// Starts the conversation named after `/go `, e.g. `/go ants`.
fn go(client: &Client) {
    client.on_message(filters::command("go"), |msg| async move {
        let name = msg.text().trim_start_matches("/go").trim().to_owned();
        msg.client().enter_conversation(&name, msg.clone())
    });
}

fn echo(client: &Client) {
    client.on_message(filters::text(), |msg| async move { msg.reply("echo").await });
}

/// Lets the conversations run up to their next wait. Under a topic scope a conversation takes presses only while a
/// wait naming their message is armed, so a press sent before that goes to the handlers.
async fn armed() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn under_a_topic_scope_a_press_counts_only_on_the_pinned_message() {
    let (client, recorder) = Client::recording();
    client
        .conversation("vote", |mut conv: Conversation, msg: Message| async move {
            msg.reply("Vote?").await?;
            // A press doesn't say its topic: the wait names the message whose buttons count, here the one that
            // started the conversation.
            let press = conv.wait_for_callback_query(filters::data("yes")).message_id(msg.id).await?;
            press.answer().text("counted").await?;
            msg.reply(format!("Voted in {}.", msg.message_thread_id.unwrap_or(0))).await?;
            Ok(())
        })
        .scope(Scope::UserInTopic);
    client.on_message(
        filters::command("vote"),
        |msg| async move { msg.client().enter_conversation("vote", msg.clone()) },
    );
    client.on_callback_query(filters::data("status"), |q| async move {
        let active = q.client().is_conversation_active("vote", &q);
        q.answer().text(if active { "active" } else { "idle" }).await
    });
    client.on_callback_query(filters::data("stop"), |q| async move {
        let ended = q.client().exit_conversation("vote", &q);
        q.answer().text(format!("ended {ended}")).await
    });
    client.on_callback_query(filters::all(), |q| async move { q.answer().text("handler").await });

    let forum = TestChat::supergroup(77).forum(true);
    let start =
        |topic: i32, id: i32| TestMessage::in_chat(forum.clone(), TestUser::new(1), "/vote").thread_id(topic).id(id);
    let press = |message: i32, data: &str| TestCallbackQuery::new(1, forum.id(), message, data).chat(forum.clone());
    soon(recorder.feed(start(5, 500))).await;
    soon(recorder.feed(start(6, 600))).await;
    texts(&recorder, forum.id(), &["Vote?", "Vote?"]).await;
    armed().await;

    // Pressed under topic 6's message: topic 5's conversation, of the same user in the same chat, doesn't take it.
    soon(recorder.feed(press(600, "yes"))).await;
    texts(&recorder, forum.id(), &["Vote?", "Vote?", "Voted in 6."]).await;
    answered(&recorder, &["counted"]).await;
    // A press on a message no wait named goes to the handlers.
    soon(recorder.feed(press(700, "yes"))).await;
    // A press finds topic 5's conversation though it doesn't say its topic, and ends it.
    soon(recorder.feed(press(700, "status"))).await;
    soon(recorder.feed(press(700, "stop"))).await;
    soon(recorder.feed(press(700, "status"))).await;
    answered(&recorder, &["counted", "handler", "active", "ended 1", "idle"]).await;
}

#[tokio::test]
async fn a_topic_scoped_wait_for_presses_must_name_its_message() {
    let (client, recorder) = Client::recording();
    let errors = errors(&client);
    client
        .conversation("loose", |mut conv: Conversation, _msg: Message| async move {
            conv.wait_for_callback_query(filters::all()).await?;
            Ok(())
        })
        .scope(Scope::ChatTopic);
    go(&client);
    let forum = TestChat::supergroup(77).forum(true);
    soon(recorder.feed(TestMessage::in_chat(forum, TestUser::new(1), "/go loose").thread_id(5))).await;
    let failed = until(&recorder, || errors.lock().unwrap().iter().any(|e| e.contains("message it names"))).await;
    assert!(failed, "{errors:?}");
}

#[tokio::test]
async fn a_chat_wide_conversation_waits_for_one_members_answer() {
    let (client, recorder) = Client::recording();
    client
        .conversation("poll", |mut conv: Conversation, msg: Message| async move {
            assert_eq!((conv.name(), conv.chat_id(), conv.user_id()), ("poll", Some(msg.chat.id), None));
            conv.client().send_message(msg.chat.id, "User 2, yes or no?").await?;
            let answer = conv.wait_from(2).await?;
            msg.reply(format!("User 2 said {}.", answer.text())).await?;
            Ok(())
        })
        .scope(Scope::Chat);
    go(&client);
    echo(&client);
    let group = TestChat::supergroup(77);

    say(&recorder, &group, 1, "/go poll").await;
    texts(&recorder, group.id(), &["User 2, yes or no?"]).await;
    // Another member's text isn't the conversation's: the echo gets it.
    say(&recorder, &group, 3, "no").await;
    texts(&recorder, group.id(), &["User 2, yes or no?", "echo"]).await;
    say(&recorder, &group, 2, "yes").await;
    texts(&recorder, group.id(), &["User 2, yes or no?", "echo", "User 2 said yes."]).await;
}

#[tokio::test]
async fn parallel_conversations_share_a_chat_and_pass_on_what_they_do_not_want() {
    let (client, recorder) = Client::recording();
    let errors = errors(&client);
    for name in ["ants", "bees"] {
        client
            .conversation(name, move |mut conv: Conversation, msg: Message| async move {
                let said = conv.wait_for(filters::text_starts_with(&name[..1])).await?;
                msg.reply(format!("{name} got {}.", said.text())).await?;
                Ok(())
            })
            .parallel();
    }
    client.conversation("crowd", |_conv: Conversation, _msg: Message| async move { Ok(()) });
    client
        .conversation("fussy", |mut conv: Conversation, _msg: Message| async move {
            conv.wait().otherwise(|_m: Message| async move { Ok(()) }).await?;
            Ok(())
        })
        .parallel();
    go(&client);

    for text in ["/go ants", "/go bees", "/go crowd", "/go fussy"] {
        soon(recorder.feed(TestMessage::private(42, text))).await;
    }
    // A conversation that isn't parallel can't join them; a parallel one can't have `otherwise`.
    let refused = until(&recorder, || {
        let errors = errors.lock().unwrap();
        errors.iter().any(|e| e.contains("already running")) && errors.iter().any(|e| e.contains("otherwise"))
    })
    .await;
    assert!(refused, "{errors:?}");

    // The oldest, ants, doesn't want "bzz": it goes on to bees.
    soon(recorder.feed(TestMessage::private(42, "bzz"))).await;
    texts(&recorder, 42, &["bees got bzz."]).await;
    soon(recorder.feed(TestMessage::private(42, "ah"))).await;
    texts(&recorder, 42, &["bees got bzz.", "ants got ah."]).await;
}

#[tokio::test(start_paused = true)]
async fn no_timeout_waits_past_the_clients_listener_timeout() {
    let (client, recorder) = Client::recording();
    client
        .conversation("slow", |mut conv: Conversation, msg: Message| async move {
            msg.reply("Take your time.").await?;
            let said = conv.wait_for(filters::text()).await?;
            msg.reply(format!("Finally: {}.", said.text())).await?;
            Ok(())
        })
        .timeout(Duration::from_secs(1))
        .no_timeout();
    go(&client);

    soon(recorder.feed(TestMessage::private(42, "/go slow"))).await;
    texts(&recorder, 42, &["Take your time."]).await;
    // Far past the 5 minutes of the client's `listener_timeout`, and the 1 second set before `no_timeout`.
    tokio::time::sleep(Duration::from_secs(3600)).await;
    soon(recorder.feed(TestMessage::private(42, "done"))).await;
    texts(&recorder, 42, &["Take your time.", "Finally: done."]).await;
}

#[tokio::test]
async fn wait_either_returns_what_came_first() {
    let (client, recorder) = Client::recording();
    let gate = Arc::new(tokio::sync::Notify::new());
    let held = Arc::clone(&gate);
    client.conversation("either", move |mut conv: Conversation, msg: Message| {
        let gate = Arc::clone(&held);
        async move {
            msg.reply("Type or press.").await?;
            // Busy for a while: the press and the text both arrive meanwhile, press first.
            gate.notified().await;
            for _ in 0..2 {
                let reply = match conv.wait_either(filters::text(), filters::data("b")).await? {
                    Answer::Message(m) => format!("typed {}", m.text()),
                    Answer::CallbackQuery(q) => format!("pressed {}", q.data),
                };
                msg.reply(reply).await?;
            }
            Ok(())
        }
    });
    go(&client);

    soon(recorder.feed(TestMessage::private(42, "/go either"))).await;
    texts(&recorder, 42, &["Type or press."]).await;
    soon(recorder.feed(TestCallbackQuery::new(42, 42, 1, "b"))).await;
    soon(recorder.feed(TestMessage::private(42, "hello"))).await;
    gate.notify_one();
    texts(&recorder, 42, &["Type or press.", "pressed b", "typed hello"]).await;
}

#[tokio::test]
async fn a_panicking_conversation_is_reported_and_frees_the_chat() {
    let (client, recorder) = Client::recording();
    let panics = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&panics);
    client.on_event(move |event| {
        if let ClientEvent::HandlerPanicked { message, .. } = event {
            sink.lock().unwrap().push(message.unwrap_or_default());
        }
    });
    let runs = Arc::new(AtomicUsize::new(0));
    client.conversation("fragile", move |_conv: Conversation, msg: Message| {
        let run = runs.fetch_add(1, Ordering::SeqCst);
        async move {
            msg.reply(format!("run {run}")).await?;
            assert!(run > 0, "boom");
            Ok(())
        }
    });
    go(&client);

    soon(recorder.feed(TestMessage::private(42, "/go fragile"))).await;
    let reported = until(&recorder, || panics.lock().unwrap().iter().any(|p| p == "boom")).await;
    assert!(reported, "{panics:?}");
    // The panic freed the chat: the conversation starts again.
    soon(recorder.feed(TestMessage::private(42, "/go fragile"))).await;
    texts(&recorder, 42, &["run 0", "run 1"]).await;
}

#[tokio::test]
async fn exit_hands_unread_updates_to_the_handlers_not_to_the_next_conversation() {
    let (client, recorder) = Client::recording();
    let gate = Arc::new(tokio::sync::Notify::new());
    let runs = Arc::new(AtomicUsize::new(0));
    client
        .conversation("hold", move |mut conv: Conversation, msg: Message| {
            let (gate, run) = (Arc::clone(&gate), runs.fetch_add(1, Ordering::SeqCst));
            async move {
                msg.reply(format!("run {run}")).await?;
                if run == 0 {
                    // Busy: what the user sends now waits in the inbox, unread, and never is.
                    gate.notified().await;
                }
                let said = conv.wait_for(filters::text()).await?;
                msg.reply(format!("got {}", said.text())).await?;
                Ok(())
            }
        })
        .ignore_commands();
    client.on_message(filters::command("restart"), |msg| async move {
        let ended = msg.client().exit_conversation("hold", &msg);
        msg.client().enter_conversation("hold", msg.clone())?;
        msg.reply(format!("restarted {ended}")).await
    });
    go(&client);
    echo(&client);

    soon(recorder.feed(TestMessage::private(42, "/go hold"))).await;
    texts(&recorder, 42, &["run 0"]).await;
    soon(recorder.feed(TestMessage::private(42, "hello"))).await;
    soon(recorder.feed(TestMessage::private(42, "/restart"))).await;
    // The first run never read "hello": it goes to the echo, not to the run that started right after.
    let sent = |text: &str| recorder.sent_texts(42).iter().any(|t| t == text);
    let handed = until(&recorder, || sent("restarted 1") && sent("run 1") && sent("echo")).await;
    assert!(handed && !sent("got hello"), "{:?}", recorder.sent_texts(42));
    soon(recorder.feed(TestMessage::private(42, "Bob"))).await;
    let answered = until(&recorder, || sent("got Bob")).await;
    assert!(answered && !sent("got hello"), "{:?}", recorder.sent_texts(42));
}

#[tokio::test]
async fn a_command_is_an_answer_unless_ignored_or_filtered_out() {
    let (client, recorder) = Client::recording();
    for name in ["plain", "strict"] {
        let flow = client.conversation(name, |mut conv: Conversation, msg: Message| async move {
            msg.reply("Name?").await?;
            let name = conv.wait_for(filters::text()).await?;
            msg.reply(format!("name {}", name.text())).await?;
            // This wait leaves `/help` to the handlers, whatever the conversation ignores.
            let topic = conv.wait_for(filters::text() & !filters::command("help")).await?;
            msg.reply(format!("topic {}", topic.text())).await?;
            Ok(())
        });
        if name == "strict" {
            flow.ignore_commands();
        }
    }
    go(&client);
    client.on_message(filters::command("cancel"), |msg| async move { msg.reply("cancelled").await });
    client.on_message(filters::command("help"), |msg| async move { msg.reply("Help.").await });

    // Not ignored: `/cancel` is the name.
    let plain = [("/go plain", "Name?"), ("/cancel", "name /cancel"), ("/help", "Help."), ("x", "topic x")];
    // Commands ignored: `/cancel` reaches its handler.
    let strict = [("/go strict", "Name?"), ("/cancel", "cancelled"), ("Ann", "name Ann")];
    for (chat, steps) in [(42, &plain[..]), (43, &strict[..])] {
        let mut expected = Vec::new();
        for &(text, reply) in steps {
            soon(recorder.feed(TestMessage::private(chat, text))).await;
            expected.push(reply);
            texts(&recorder, chat, &expected).await;
        }
    }
}
