// Run with `cargo test --test live_replace_callback_message -- --ignored --test-threads=1 --nocapture`; see
// tests/support/mod.rs for the env. Like every live test, these use the kit's shared session files, so run them with
// `--test-threads=1`.
//
// No human is needed: the kit's user presses the bot's buttons through the library.

#[allow(dead_code)]
mod support;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc::{self, UnboundedReceiver};
use vaegram::types::{
    CallbackQuery, EphemeralMessage, InlineKeyboardButton, InlineKeyboardMarkup, InputMedia, InputRichMessage, Text,
    Upload,
};
use vaegram::{Client, Error, filters};

const WAIT: Duration = Duration::from_secs(30);

fn nonce() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

/// Waits up to [`WAIT`] for the first item `rx` brings that `pick` takes; every other item is dropped. `None` on
/// timeout.
async fn next<T>(rx: &mut UnboundedReceiver<T>, mut pick: impl FnMut(&T) -> bool) -> Option<T> {
    let found = tokio::time::timeout(WAIT, async {
        loop {
            match rx.recv().await {
                Some(item) if pick(&item) => return item,
                Some(_) => {}
                None => panic!("the handler feeding this channel is gone"),
            }
        }
    });
    found.await.ok()
}

/// Waits up to [`WAIT`] for the receiver's copy of `sent`, the bot's copy of an ephemeral message: the one with the
/// same id in the same chat, since every live run so far gave both copies the same id. A copy that came before the
/// send returned is still waiting in `seen`, so the order doesn't matter. `Err` holds what arrived instead.
async fn receive(
    seen: &mut UnboundedReceiver<EphemeralMessage>,
    sent: &EphemeralMessage,
) -> Result<EphemeralMessage, Vec<EphemeralMessage>> {
    let mut others = Vec::new();
    let found = tokio::time::timeout(WAIT, async {
        loop {
            match seen.recv().await {
                Some(m) if m.id == sent.id && m.chat.id == sent.chat.id => return m,
                Some(m) => others.push(m),
                None => panic!("the handler feeding this channel is gone"),
            }
        }
    })
    .await;
    found.map_err(|_| others)
}

/// Why the receiver's copy of `sent` wasn't found: a message with its `text` arrived under another id, or nothing
/// of it arrived. `text` is `None` for a rich message, whose copy can only be found by id.
fn missing(sent: &EphemeralMessage, text: Option<&str>, others: &[EphemeralMessage]) -> Outcome {
    let arrived: Vec<String> = others.iter().map(fields).collect();
    let details = format!("the bot's copy [{}]; arrived instead {arrived:?}", fields(sent));
    match others.iter().find(|m| text.is_some_and(|text| m.text == text)) {
        Some(m) => Outcome::IdsDiffer(format!("the user's copy has id {}, the bot's {}; {details}", m.id, sent.id)),
        None => Outcome::Nothing(details),
    }
}

/// The fields of an ephemeral message that say where it is shown.
fn fields(m: &EphemeralMessage) -> String {
    format!(
        "id {}, chat {}, anchor_message_id {:?}, reply to {:?}, thread {:?}, text {:?}",
        m.id, m.chat.id, m.anchor_message_id, m.reply_to_message_id, m.message_thread_id, m.text
    )
}

/// What the bot sends in answer to a press. Every kind asks for the replace unless it says otherwise.
#[derive(Clone)]
enum How {
    /// Plain text to the presser.
    Plain { replace: bool },
    /// Plain text to the presser, replying to the regular message `target`, without the replace: with it, awaiting
    /// fails before any request is sent.
    ReplyTo(i32),
    /// A rich message to the presser.
    Rich,
    /// A document to the presser, with the answer's text as its caption.
    Media,
    /// Plain text to this user, who didn't press.
    To(i64),
}

/// Sends the ephemeral answer `how` asks for to `query`.
async fn answer(query: &CallbackQuery, how: How) -> vaegram::Result<EphemeralMessage> {
    let text = Text::new().plain(format!("ephemeral answer to {}", query.data).as_str());
    let send = match how {
        How::Plain { replace: false } => return query.answer_ephemeral(text)?.await,
        How::Plain { replace: true } => query.answer_ephemeral(text)?,
        How::ReplyTo(target) => return query.answer_ephemeral(text)?.reply_to(target).await,
        How::Rich => {
            let rich = InputRichMessage::html(format!("<b>rich</b> ephemeral answer to {}", query.data));
            query.answer_ephemeral(rich)?
        }
        How::Media => {
            let file = InputMedia::document(Upload::bytes("vaegram.txt", b"vaegram live test")).caption(text);
            query.answer_ephemeral("")?.media(file)
        }
        How::To(receiver) => {
            let chat = query.chat().ok_or(Error::InvalidArgument("the press has no chat"))?.id;
            query.client().send_ephemeral_message(chat, receiver, text).callback_query_id(query.id)
        }
    };
    send.replace_callback_query_message().await
}

/// What the bot's handler did with one press: the button's data, the ephemeral answer it sent, and the
/// `answer_callback_query` it made after that.
type Answer = (String, vaegram::Result<EphemeralMessage>, vaegram::Result<()>);

/// What a test leaves behind: the bot's messages by chat, and its ephemeral answers by chat, receiver and id. The
/// bot's handler records an answer as soon as its send returns, so one that comes after the test stopped waiting is
/// removed too; `in_flight` counts the handler's sends that haven't returned yet.
#[derive(Default)]
struct Made {
    messages: Vec<(i64, i32)>,
    ephemerals: Vec<(i64, i64, i32)>,
    in_flight: usize,
}

/// Waits up to [`WAIT`] for the bot's handler to finish the sends it started, so that `clean` sees what they made.
/// `Some` says how many were still going.
async fn settle(made: &Mutex<Made>) -> Option<String> {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let in_flight = made.lock().unwrap().in_flight;
        if in_flight == 0 {
            return None;
        }
        if tokio::time::Instant::now() >= deadline {
            return Some(format!("{in_flight} ephemeral answers were still being sent after {WAIT:?}; check the chat"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Removes everything `made` holds and returns what couldn't be removed. A FLOOD_WAIT is only reported.
async fn clean(made: &Mutex<Made>, bot: &Client) -> Vec<String> {
    let (ephemerals, messages) = {
        let mut made = made.lock().unwrap();
        (std::mem::take(&mut made.ephemerals), std::mem::take(&mut made.messages))
    };
    let mut failed = Vec::new();
    let mut note = |result: vaegram::Result<()>, what: String| match result {
        Ok(()) => {}
        Err(e) if e.flood_wait().is_some() => eprintln!("skipped the cleanup: {what} is flood-limited; do it by hand"),
        Err(e) => failed.push(format!("{what}: {e:?}")),
    };
    for (chat_id, receiver, id) in ephemerals {
        let what = format!("deleting ephemeral answer {id} in {chat_id}");
        note(bot.delete_ephemeral_message(chat_id, receiver, id).await, what);
    }
    for (chat_id, id) in messages {
        note(bot.delete_messages(chat_id, &[id]).await, format!("deleting message {id} in {chat_id}"));
    }
    failed
}

/// What a test ends with: waits for the handler's sends, then removes what was made. Returns what went wrong.
async fn finish(made: &Mutex<Made>, bot: &Client) -> Vec<String> {
    let mut failed: Vec<String> = settle(made).await.into_iter().collect();
    failed.extend(clean(made, bot).await);
    failed
}

/// What every press needs: the two clients, what the bot sends for each button, what was made, and the bot's
/// reports.
struct Ctx {
    user: Client,
    bot: Client,
    plan: Arc<Mutex<HashMap<String, How>>>,
    made: Arc<Mutex<Made>>,
    answers: UnboundedReceiver<Answer>,
}

/// How far a press got: the pressed message, the user's click, and what the bot's handler did.
struct Pressed {
    button: i32,
    click: String,
    sent: vaegram::Result<EphemeralMessage>,
    answered: vaegram::Result<()>,
}

/// Why a press didn't get as far as the bot's answer.
enum Stuck {
    Telegram(&'static str, Error),
    Missing(String),
}

impl Stuck {
    fn describe(&self) -> String {
        match self {
            Stuck::Telegram(what, e) => format!("{what} failed: {e:?}"),
            Stuck::Missing(what) => what.clone(),
        }
    }
}

impl Ctx {
    /// Has the bot answer every press of a button whose data starts with `prefix` the way the plan says.
    fn new(user: Client, bot: Client, prefix: &str) -> Ctx {
        let plan: Arc<Mutex<HashMap<String, How>>> = Arc::default();
        let made: Arc<Mutex<Made>> = Arc::default();
        let (answer_tx, answers) = mpsc::unbounded_channel();
        let (handler_plan, handler_made) = (plan.clone(), made.clone());
        bot.on_callback_query(filters::data_prefix(prefix), move |query| {
            let (answer_tx, plan, made) = (answer_tx.clone(), handler_plan.clone(), handler_made.clone());
            async move {
                made.lock().unwrap().in_flight += 1;
                let how = plan.lock().unwrap().get(&query.data).cloned();
                let sent = match how {
                    Some(how) => answer(&query, how).await,
                    None => Err(Error::InvalidArgument("no answer is planned for this button")),
                };
                {
                    let mut made = made.lock().unwrap();
                    if let Ok(m) = &sent {
                        made.ephemerals.push((m.chat.id, m.receiver_user.id, m.id));
                    }
                    made.in_flight -= 1;
                }
                let answered = query.answer().await;
                let _ = answer_tx.send((query.data.clone(), sent, answered));
                Ok(())
            }
        })
        .group(1);
        Ctx { user, bot, plan, made, answers }
    }

    /// Posts a button carrying `data` in `chat_id` (in `topic` when given), has the user press it, and waits for
    /// what the bot's handler did, which `how` decides.
    async fn press(&mut self, chat_id: i64, topic: Option<i32>, data: &str, how: How) -> Result<Pressed, Stuck> {
        self.plan.lock().unwrap().insert(data.to_owned(), how);
        let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Tap", data)]]);
        let text = Text::new().plain(format!("vaegram live test: press {data}").as_str());
        let send = self.bot.send_message(chat_id, text).reply_markup(keyboard);
        let send = match topic {
            Some(topic) => send.message_thread_id(topic),
            None => send,
        };
        let button = send.await.map_err(|e| Stuck::Telegram("posting the button", e))?;
        self.made.lock().unwrap().messages.push((chat_id, button.id));

        // The user presses the button on its own copy of the message, the way a person's app does.
        let found = self.user.get_messages(chat_id, &[button.id]).await;
        let found = found.map_err(|e| Stuck::Telegram("the user reading the button's message", e))?;
        let Some(pressed) = found.into_iter().next().flatten() else {
            return Err(Stuck::Missing("the user can't read the button's message".into()));
        };
        let click = tokio::spawn(async move { tokio::time::timeout(WAIT, async { pressed.click("Tap").await }).await });

        let answer = next(&mut self.answers, |(answered_data, _, _)| *answered_data == data).await;
        let click = format!("{:?}", click.await);
        let Some((_, sent, answered)) = answer else {
            return Err(Stuck::Missing(format!("the bot got no press within {WAIT:?}; the user's click: {click}")));
        };
        Ok(Pressed { button: button.id, click, sent, answered })
    }
}

/// One press and its plain ephemeral answer: the pressed message's id, the bot's copy of the answer and the user's.
struct Tap {
    button: i32,
    sent: EphemeralMessage,
    seen: EphemeralMessage,
}

/// A press answered with plain text, with `replace_callback_query_message` when `data` ends in ":replace". `Err` is
/// the bot's failed send, or a FLOOD_WAIT on posting the button.
async fn tap(
    ctx: &mut Ctx,
    chat_id: i64,
    data: String,
    seen: &mut UnboundedReceiver<EphemeralMessage>,
) -> Result<Tap, Error> {
    let how = How::Plain { replace: data.ends_with(":replace") };
    let pressed = match ctx.press(chat_id, None, &data, how).await {
        Ok(pressed) => pressed,
        Err(Stuck::Telegram(_, e)) if e.flood_wait().is_some() => return Err(e),
        Err(stuck) => panic!("{data}: {}", stuck.describe()),
    };
    eprintln!("{data}: the user's click returned {}", pressed.click);
    eprintln!("{data}: the bot's answer_callback_query after the ephemeral send returned {:?}", pressed.answered);
    let sent = pressed.sent?;
    eprintln!("{data}: the bot's copy of its ephemeral answer: {}", fields(&sent));

    // Found by the id the bot's send returned; the text, which holds the press's nonce, is checked on its own.
    let text = format!("ephemeral answer to {data}");
    let on_user = match receive(seen, &sent).await {
        Ok(on_user) => on_user,
        Err(others) => panic!("{data}: {}", missing(&sent, Some(text.as_str()), &others).line()),
    };
    eprintln!("{data}: the user's copy of the ephemeral answer: {}", fields(&on_user));
    assert_eq!(on_user.text, text, "{data}: the user's copy with the bot's id has other text");
    Ok(Tap { button: pressed.button, sent, seen: on_user })
}

/// Says so and returns `true` when Telegram refused an ephemeral message for want of a right: 403, or a name that
/// says so. The Bot API lets a bot send one within 15 seconds of a callback query, and at other times only as an
/// administrator.
fn refused_for_rights(error: &Error) -> bool {
    let Some(rpc) = error.rpc() else { return false };
    let refused = rpc.code == 403 || ["ADMIN", "RIGHT", "FORBIDDEN"].iter().any(|word| rpc.name.contains(word));
    if refused {
        eprintln!("skipped: Telegram refused the bot's ephemeral answer to a press in VAEGRAM_CHAT_ID: {rpc:?}");
    }
    refused
}

async fn replace_steps(mut ctx: Ctx, chat_id: i64, prefix: String, mut seen: UnboundedReceiver<EphemeralMessage>) {
    let flood = "posting the button or its ephemeral answer";
    // The control: the same press and answer without the setter. If Telegram refuses even this, the bot can't
    // answer presses with ephemeral messages in this chat and the replace can't be judged.
    let control = match tap(&mut ctx, chat_id, format!("{prefix}:control"), &mut seen).await {
        Ok(tap) => tap,
        Err(e) if support::flood_limited(&e, flood) || refused_for_rights(&e) => return,
        Err(e) => panic!("the ephemeral answer without replace_callback_query_message failed: {e:?}"),
    };
    let replaced = match tap(&mut ctx, chat_id, format!("{prefix}:replace"), &mut seen).await {
        Ok(tap) => tap,
        Err(e) if support::flood_limited(&e, flood) => return,
        Err(e) => panic!("Telegram took the answer without replace_callback_query_message but refused it with: {e:?}"),
    };

    let (control_seen, replaced_seen) = (control.seen.anchor_message_id, replaced.seen.anchor_message_id);
    eprintln!("anchor_message_id the user saw: control {control_seen:?}, replace {replaced_seen:?}");
    eprintln!("the pressed message of the replace run: {}", replaced.button);
    // Seen on 2026-11-13: the sender's copy has None here even after a replace. Printed, not asserted: the replace
    // is judged by what the receiver sees.
    eprintln!(
        "anchor_message_id in the bot's copies: control {:?}, replace {:?}",
        control.sent.anchor_message_id, replaced.sent.anchor_message_id
    );
    assert_eq!(control_seen, None, "the answer sent without the anchor flag is anchored too");
    assert_eq!(replaced_seen, Some(replaced.button), "the answer sent with the anchor flag isn't on the pressed one");
}

/// The user's ephemeral messages, from a handler in group 1.
fn ephemerals_of(client: &Client) -> UnboundedReceiver<EphemeralMessage> {
    let (seen_tx, seen) = mpsc::unbounded_channel();
    client
        .on_ephemeral_message(filters::all(), move |m| {
            let _ = seen_tx.send(m);
            async { Ok(()) }
        })
        .group(1);
    seen
}

#[tokio::test]
#[ignore = "needs the kit bot and user in VAEGRAM_CHAT_ID, a supergroup"]
async fn ephemeral_replaces_the_callback_message() {
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a supergroup with the kit bot and user)");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let Some(kit) = support::Kit::from_env().await else { return };
    let prefix = format!("vaegram-replace-{}", nonce());
    let ctx = Ctx::new(kit.user.client.clone(), kit.bot.clone(), &prefix);
    let seen = ephemerals_of(&kit.user.client);
    let (made, bot) = (ctx.made.clone(), ctx.bot.clone());

    let steps = tokio::spawn(replace_steps(ctx, chat_id, prefix, seen)).await;

    let failed = finish(&made, &bot).await;
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
    assert!(failed.is_empty(), "the cleanup failed: {failed:?}");
}

/// What came of one press and its answer.
enum Outcome {
    /// An answer reached the receiver as `got`; `anchored` when its `anchor_message_id` is the pressed message.
    Arrived { anchored: bool, got: Box<EphemeralMessage>, details: String },
    /// The bot's send failed with this error.
    Refused(Error),
    /// The send went through and nothing with its id, or its text, reached the receiver.
    Nothing(String),
    /// A message with the answer's text reached the receiver under another id than the bot's copy has.
    IdsDiffer(String),
    /// The receiver's copy with the bot's id has other text than the answer.
    OtherText(String),
    /// The press didn't get as far as the bot's answer.
    Setup(Stuck),
}

impl Outcome {
    /// The outcome in one line: `anchored`, `not anchored`, `refused: <error>`, `nothing arrived`, `ids differ`,
    /// `other text` or `setup failed`.
    fn line(&self) -> String {
        match self {
            Outcome::Arrived { anchored: true, details, .. } => format!("anchored: {details}"),
            Outcome::Arrived { anchored: false, details, .. } => format!("not anchored: {details}"),
            Outcome::Refused(e) => format!("refused: {e:?}"),
            Outcome::Nothing(details) => format!("nothing arrived within {WAIT:?}: {details}"),
            Outcome::IdsDiffer(details) => format!("ids differ: {details}"),
            Outcome::OtherText(details) => format!("other text: {details}"),
            Outcome::Setup(stuck) => format!("setup failed: {}", stuck.describe()),
        }
    }

    fn flooded(&self) -> bool {
        match self {
            Outcome::Refused(e) | Outcome::Setup(Stuck::Telegram(_, e)) => e.flood_wait().is_some(),
            _ => false,
        }
    }

    fn anchored(&self) -> bool {
        matches!(self, Outcome::Arrived { anchored: true, .. })
    }

    /// The name of Telegram's error when it refused the send.
    fn telegram_error(&self) -> Option<&str> {
        match self {
            Outcome::Refused(e) => e.rpc().map(|rpc| rpc.name.as_str()),
            _ => None,
        }
    }
}

/// Prints `outcome` under `name` and, unless Telegram flood-limited it, records it in `problems` when it isn't `ok`.
/// A failed combination doesn't stop the others.
fn judge(problems: &mut Vec<String>, name: &str, outcome: &Outcome, ok: bool) {
    eprintln!("{name}: {}", outcome.line());
    if outcome.flooded() {
        eprintln!("{name}: not judged, flood-limited");
    } else if !ok {
        problems.push(format!("{name}: {}", outcome.line()));
    }
}

/// Presses a button and answers it the way `how` says. `seen` brings the receiver's ephemeral messages.
async fn outcome(
    ctx: &mut Ctx,
    chat_id: i64,
    topic: Option<i32>,
    data: &str,
    how: How,
    seen: &mut UnboundedReceiver<EphemeralMessage>,
) -> Outcome {
    while seen.try_recv().is_ok() {}
    // A rich message's body isn't exposed by `EphemeralMessage` (it has no rich field, and `text` arrives empty), so
    // a rich answer is found by id alone and its nonce can't be checked.
    let text = match how {
        How::Rich => None,
        _ => Some(format!("ephemeral answer to {data}")),
    };
    let pressed = match ctx.press(chat_id, topic, data, how).await {
        Ok(pressed) => pressed,
        Err(stuck) => return Outcome::Setup(stuck),
    };
    let sent = match pressed.sent {
        Ok(sent) => sent,
        Err(e) => return Outcome::Refused(e),
    };
    let got = match receive(seen, &sent).await {
        Ok(got) => got,
        Err(others) => return missing(&sent, text.as_deref(), &others),
    };
    if let Some(text) = text.filter(|text| got.text != *text) {
        return Outcome::OtherText(format!("expected {text:?}; received [{}]", fields(&got)));
    }
    let details = format!(
        "pressed {}; received [{}]; the bot's copy [{}]; click {}; answer_callback_query {:?}",
        pressed.button,
        fields(&got),
        fields(&sent),
        pressed.click,
        pressed.answered
    );
    Outcome::Arrived { anchored: got.anchor_message_id == Some(pressed.button), got: Box::new(got), details }
}

/// A second logged-in user, its id and its ephemeral messages. Its run stops when it drops.
struct Second {
    id: i64,
    seen: UnboundedReceiver<EphemeralMessage>,
    _disconnect: support::Disconnect,
}

/// Logs in `VAEGRAM_SECOND_USER_SESSION` and runs it. `Err` says why it can't be used.
async fn second_user(session: String) -> Result<Second, String> {
    let env = |k: &str| std::env::var(k).map_err(|_| format!("set {k}"));
    let api_id: i32 = env("VAEGRAM_API_ID")?.parse().map_err(|e| format!("VAEGRAM_API_ID: {e}"))?;
    let builder = Client::builder(session, api_id, env("VAEGRAM_API_HASH")?).catch_up(false);
    let client = builder.connect().await.map_err(|e| format!("logging in the second user: {e:?}"))?;
    let disconnect = support::Disconnect(vec![client.clone()]);
    let seen = ephemerals_of(&client);
    let (started_tx, mut started) = mpsc::unbounded_channel();
    client.on_start(move |_| {
        let _ = started_tx.send(());
        async { Ok(()) }
    });
    let runner = client.clone();
    tokio::spawn(async move { runner.run().await });
    if tokio::time::timeout(WAIT, started.recv()).await.is_err() {
        return Err("the second user's run didn't start".into());
    }
    let id = client.get_me().await.map_err(|e| format!("get_me for the second user: {e:?}"))?.id;
    Ok(Second { id, seen, _disconnect: disconnect })
}

/// Runs every combination, prints one line for each, and returns what went wrong: a combination that didn't do what
/// is known of it, or a cleanup that failed.
async fn option_steps(
    mut ctx: Ctx,
    chat_id: i64,
    prefix: String,
    mut seen: UnboundedReceiver<EphemeralMessage>,
) -> Vec<String> {
    let mut problems = Vec::new();

    // (a) Telegram answered REPLY_TO_INVALID to reply_to together with replace, while the same reply alone went
    // through with `reply to` naming the target (2026-11-13). The pair is now refused locally (a unit test covers
    // it); the reply alone is checked here, so the refusal stays tied to the pair.
    let reply = "(a) reply_to another regular message, without replace";
    let target = ctx.bot.send_message(chat_id, Text::new().plain("vaegram live test: the answer replies here")).await;
    match target {
        Ok(target) => {
            ctx.made.lock().unwrap().messages.push((chat_id, target.id));
            let data = format!("{prefix}:reply");
            let control = outcome(&mut ctx, chat_id, None, &data, How::ReplyTo(target.id), &mut seen).await;
            let replied = |got: &EphemeralMessage| got.reply_to_message_id == Some(target.id);
            let ok = matches!(&control, Outcome::Arrived { anchored: false, got, .. } if replied(got));
            judge(&mut problems, reply, &control, ok);
        }
        Err(e) => {
            let posting = Outcome::Setup(Stuck::Telegram("posting the message to reply to", e));
            judge(&mut problems, reply, &posting, false);
        }
    }
    problems.extend(finish(&ctx.made, &ctx.bot).await);

    // Found by id alone: `EphemeralMessage` doesn't expose a rich body to check the nonce in (see `outcome`).
    let rich = outcome(&mut ctx, chat_id, None, &format!("{prefix}:rich"), How::Rich, &mut seen).await;
    judge(&mut problems, "(b) replace + a rich message", &rich, rich.anchored());
    problems.extend(finish(&ctx.made, &ctx.bot).await);

    // A photo needs real image bytes, which the tests don't carry; a document from memory takes the same media path.
    let media = outcome(&mut ctx, chat_id, None, &format!("{prefix}:media"), How::Media, &mut seen).await;
    let ok = matches!(&media, Outcome::Arrived { anchored: true, got, .. } if got.media.is_some());
    judge(&mut problems, "(c) replace + media (a document from memory)", &media, ok);
    problems.extend(finish(&ctx.made, &ctx.bot).await);

    // (d) A plain replace answering a button posted inside a forum topic, with no thread set on the answer (a thread
    // next to replace is refused locally). With the env set it must come out anchored.
    let topic_press = "(d) replace answering a press in a forum topic";
    match std::env::var("VAEGRAM_FORUM_CHAT_ID").map(|id| id.parse::<i64>()) {
        Ok(Ok(forum)) => {
            let result = match ctx.user.create_forum_topic(forum, "vaegram live test").await {
                Ok(topic) => {
                    let (data, how) = (format!("{prefix}:topic"), How::Plain { replace: true });
                    let result = outcome(&mut ctx, forum, Some(topic.id), &data, how, &mut seen).await;
                    problems.extend(finish(&ctx.made, &ctx.bot).await);
                    if let Err(e) = ctx.user.delete_forum_topic(forum, topic.id).await {
                        problems.push(format!("deleting topic {} in {forum}: {e:?}", topic.id));
                    }
                    result
                }
                Err(e) => Outcome::Setup(Stuck::Telegram("the user creating a topic in VAEGRAM_FORUM_CHAT_ID", e)),
            };
            judge(&mut problems, topic_press, &result, result.anchored());
        }
        Ok(Err(e)) => {
            let result = Outcome::Setup(Stuck::Missing(format!("VAEGRAM_FORUM_CHAT_ID: {e}")));
            judge(&mut problems, topic_press, &result, false);
        }
        Err(_) => eprintln!("{topic_press}: skipped: set VAEGRAM_FORUM_CHAT_ID (a forum supergroup)"),
    }

    eprintln!(
        "(e) replace on a button of an ephemeral message: skipped: vaegram can't press one. Message::click takes a \
         regular Message, and ephemeral.getCallbackAnswer has no method"
    );

    // (f) Anchored or refused by Telegram, either is accepted, and the error's name is printed. A refusal proves only
    // that the bot learns of it instead of the answer going nowhere silently; it says nothing about support.
    let other = "(f) replace + a receiver who didn't press";
    match std::env::var("VAEGRAM_SECOND_USER_SESSION") {
        Ok(session) => {
            let result = match second_user(session).await {
                Ok(mut second) => {
                    // The bot learns the second user's access hash from the chat's member list.
                    match ctx.bot.get_chat_member(chat_id, second.id).await {
                        Ok(_) => {
                            let data = format!("{prefix}:other");
                            outcome(&mut ctx, chat_id, None, &data, How::To(second.id), &mut second.seen).await
                        }
                        Err(e) => Outcome::Setup(Stuck::Telegram("the bot looking up the second user in the chat", e)),
                    }
                }
                Err(why) => Outcome::Setup(Stuck::Missing(why)),
            };
            let ok = result.anchored() || result.telegram_error().is_some();
            judge(&mut problems, other, &result, ok);
            if let Some(name) = result.telegram_error() {
                eprintln!("{other}: Telegram's error: {name}");
            }
            problems.extend(finish(&ctx.made, &ctx.bot).await);
        }
        Err(_) => eprintln!("{other}: skipped: set VAEGRAM_SECOND_USER_SESSION"),
    }
    problems
}

#[tokio::test]
#[ignore = "needs the kit bot and user in VAEGRAM_CHAT_ID; checks what replace does next to other options"]
async fn replace_with_other_options() {
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a supergroup with the kit bot and user)");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let Some(kit) = support::Kit::from_env().await else { return };
    let prefix = format!("vaegram-options-{}", nonce());
    let ctx = Ctx::new(kit.user.client.clone(), kit.bot.clone(), &prefix);
    let seen = ephemerals_of(&kit.user.client);
    let (made, bot) = (ctx.made.clone(), ctx.bot.clone());

    let steps = tokio::spawn(option_steps(ctx, chat_id, prefix, seen)).await;

    let mut problems = finish(&made, &bot).await;
    match steps {
        Ok(found) => problems.extend(found),
        Err(panicked) => std::panic::resume_unwind(panicked.into_panic()),
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
