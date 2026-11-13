// Run with `cargo test --test live_replace_callback_message -- --ignored --test-threads=1 --nocapture`; see
// tests/support/mod.rs for the env. These tests share session files, so run with `--test-threads=1`.
//
// No human is needed: the kit's user presses the bot's button through the library.

#[allow(dead_code)]
mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc::{self, UnboundedReceiver};
use vaegram::types::{EphemeralMessage, InlineKeyboardButton, InlineKeyboardMarkup, Message, Text};
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

/// The fields of an ephemeral message that say where it is shown.
fn fields(m: &EphemeralMessage) -> String {
    format!(
        "id {}, chat {}, anchor_message_id {:?}, reply to {:?}, thread {:?}, text {:?}",
        m.id, m.chat.id, m.anchor_message_id, m.reply_to_message_id, m.message_thread_id, m.text
    )
}

/// What the bot's callback handler did with one press: the button's data, the ephemeral answer it sent, and the
/// `answer_callback_query` it made after that.
type Answer = (String, vaegram::Result<EphemeralMessage>, vaegram::Result<()>);

/// What the test leaves in the chat, so every path can remove it.
#[derive(Default)]
struct Made {
    buttons: Vec<i32>,
    ephemerals: Vec<i32>,
}

/// One press and its ephemeral answer: the pressed message's id, the bot's copy of the answer and the user's.
struct Tap {
    button: i32,
    sent: EphemeralMessage,
    seen: EphemeralMessage,
}

/// The two clients and the chat the test works in.
struct Ends {
    user: Client,
    bot: Client,
    chat_id: i64,
}

/// The bot posts a button carrying `data`, the user presses it, and the bot answers the press with an ephemeral
/// message: with `replace_callback_query_message` when `data` ends in ":replace". `Err` is the bot's failed send.
async fn tap(
    ends: &Ends,
    data: String,
    made: &Mutex<Made>,
    answers: &mut UnboundedReceiver<Answer>,
    seen: &mut UnboundedReceiver<EphemeralMessage>,
) -> Result<Tap, Error> {
    let Ends { user, bot, chat_id } = ends;
    let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Tap", data.clone())]]);
    let text = Text::new().plain(format!("vaegram live test: press {data}").as_str());
    let button = bot.send_message(*chat_id, text).reply_markup(keyboard).await.unwrap();
    made.lock().unwrap().buttons.push(button.id);

    // The user presses the button on its own copy of the message, the way a person's app does.
    let pressed = user.get_messages(*chat_id, &[button.id]).await.unwrap().into_iter().next().flatten();
    let pressed: Message = pressed.expect("the user can't read the bot's message in VAEGRAM_CHAT_ID");
    let click = tokio::spawn(async move { tokio::time::timeout(WAIT, async { pressed.click("Tap").await }).await });

    let answer = next(answers, |(answered_data, _, _)| *answered_data == data).await;
    let (_, sent, answered) = answer.unwrap_or_else(|| panic!("the bot got no press of {data} within {WAIT:?}"));
    let click = click.await.unwrap();
    eprintln!("{data}: the user's click returned {click:?}");
    eprintln!("{data}: the bot's answer_callback_query after the ephemeral send returned {answered:?}");
    let sent = sent?;
    made.lock().unwrap().ephemerals.push(sent.id);
    eprintln!("{data}: the bot's copy of its ephemeral answer: {}", fields(&sent));

    // Matched by text, which holds the press's nonce: whether both sides number an ephemeral message alike is not
    // assumed.
    let on_user = next(seen, |m| m.text == sent.text).await;
    let on_user = on_user.unwrap_or_else(|| panic!("{data}: the user got no ephemeral answer within {WAIT:?}"));
    eprintln!("{data}: the user's copy of the ephemeral answer: {}", fields(&on_user));
    Ok(Tap { button: button.id, sent, seen: on_user })
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

async fn replace_steps(
    ends: Ends,
    prefix: String,
    made: Arc<Mutex<Made>>,
    mut answers: UnboundedReceiver<Answer>,
    mut seen: UnboundedReceiver<EphemeralMessage>,
) {
    // The control: the same press and answer without the setter. If Telegram refuses even this, the bot can't
    // answer presses with ephemeral messages in this chat and the replace can't be judged.
    let control = match tap(&ends, format!("{prefix}:control"), &made, &mut answers, &mut seen).await {
        Ok(tap) => tap,
        Err(e) if support::flood_limited(&e, "an ephemeral answer") || refused_for_rights(&e) => return,
        Err(e) => panic!("the ephemeral answer without replace_callback_query_message failed: {e:?}"),
    };
    let replaced = match tap(&ends, format!("{prefix}:replace"), &made, &mut answers, &mut seen).await {
        Ok(tap) => tap,
        Err(e) if support::flood_limited(&e, "an ephemeral answer") => return,
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

#[tokio::test]
#[ignore = "needs the kit bot and user in VAEGRAM_CHAT_ID, a supergroup"]
async fn ephemeral_replaces_the_callback_message() {
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a supergroup with the kit bot and user)");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let Some(kit) = support::Kit::from_env().await else { return };
    let (user, bot) = (kit.user.client.clone(), kit.bot.clone());
    let user_id = user.get_me().await.unwrap().id;
    let prefix = format!("vaegram-replace-{}", nonce());

    let (answer_tx, answers) = mpsc::unbounded_channel();
    bot.on_callback_query(filters::data_prefix(&prefix), move |query| {
        let answer_tx = answer_tx.clone();
        async move {
            let text = Text::new().plain(format!("ephemeral answer to {}", query.data).as_str());
            let sent = match query.answer_ephemeral(text) {
                Ok(send) if query.data.ends_with(":replace") => send.replace_callback_query_message().await,
                Ok(send) => send.await,
                Err(e) => Err(e),
            };
            let answered = query.answer().await;
            let _ = answer_tx.send((query.data.clone(), sent, answered));
            Ok(())
        }
    })
    .group(1);
    let (seen_tx, seen) = mpsc::unbounded_channel();
    user.on_ephemeral_message(filters::all(), move |m| {
        let _ = seen_tx.send(m);
        async { Ok(()) }
    })
    .group(1);

    let made = Arc::new(Mutex::new(Made::default()));
    let ends = Ends { user: user.clone(), bot: bot.clone(), chat_id };
    let steps = tokio::spawn(replace_steps(ends, prefix, made.clone(), answers, seen)).await;

    let made = std::mem::take(&mut *made.lock().unwrap());
    for id in made.ephemerals {
        support::cleanup(bot.delete_ephemeral_message(chat_id, user_id, id).await, "deleting an ephemeral answer");
    }
    if !made.buttons.is_empty() {
        support::cleanup(bot.delete_messages(chat_id, &made.buttons).await, "deleting the button messages");
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}
