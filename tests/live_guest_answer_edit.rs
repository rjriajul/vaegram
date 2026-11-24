// Run with `cargo test --test live_guest_answer_edit -- --ignored --test-threads=1 --nocapture`; see
// tests/support/mod.rs for the env. These tests share session files, so run with `--test-threads=1`.
//
// No human is needed: the kit's user mentions the bot through the library.

#[allow(dead_code)]
mod support;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc::{self, UnboundedReceiver};
use vaegram::types::{InlineQueryResultArticle, Text};
use vaegram::{Client, filters};

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

async fn guest_steps(user: Client, bot: Client, group: i64, username: String) {
    let nonce = nonce();
    let (guest_tx, mut guests) = mpsc::unbounded_channel();
    bot.on_guest_message(filters::all(), move |guest| {
        let _ = guest_tx.send(guest);
        async { Ok(()) }
    })
    .group(1);
    let (seen_tx, mut seen) = mpsc::unbounded_channel();
    let new_tx = seen_tx.clone();
    user.on_message(filters::chat(group), move |m| {
        if !m.outgoing {
            let _ = new_tx.send((false, m));
        }
        async { Ok(()) }
    })
    .group(1);
    user.on_edited_message(filters::chat(group), move |m| {
        let _ = seen_tx.send((true, m));
        async { Ok(()) }
    })
    .group(1);

    // A guest query is an ordinary message that mentions the bot in a chat the bot isn't in: Telegram turns it into
    // updateBotGuestChatQuery for the bot. There is no other user-side call.
    let mention = format!("@{username} vaegram guest test {nonce}");
    let summon = user.send_message(group, Text::new().plain(mention.as_str())).await.unwrap();
    let kinds: Vec<_> = summon.entities().unwrap_or_default().iter().map(|e| format!("{:?}", e.kind)).collect();
    eprintln!("the user's mention {}: entities {kinds:?}", summon.id);

    let nonce_text = nonce.to_string();
    let Some(guest) = next(&mut guests, |g| g.message.text().contains(&nonce_text)).await else {
        panic!("no guest query reached the bot within {WAIT:?} of the user mentioning it in a chat it isn't in");
    };
    eprintln!(
        "guest query: chat {}, message {}, caller user {:?}, caller chat {:?}",
        guest.message.chat.id,
        guest.message.id,
        guest.caller_user.as_ref().map(|u| u.id),
        guest.caller_chat.as_ref().map(|c| c.id)
    );

    // answer_text sends an article without a title. If Telegram refuses that, the edit is still tried on a titled
    // answer, and the test fails at the end.
    let first = format!("guest answer {nonce}");
    let (sent, untitled) = match guest.answer_text(first.as_str()).await {
        Ok(sent) => {
            eprintln!("answer_text (an article with no title) was accepted: {sent:?}");
            (sent, None)
        }
        Err(e) if support::flood_limited(&e, "answering a guest query") => return,
        Err(e) => {
            eprintln!("answer_text (an article with no title) was refused: {e:?}; answering with a titled article");
            let titled = guest.answer(InlineQueryResultArticle::new("vaegram", first.as_str())).await;
            (titled.expect("a titled answer to the guest query failed too"), Some(e))
        }
    };

    let answer = next(&mut seen, |(edited, m)| !edited && m.text() == first).await;
    let (_, answer) =
        answer.unwrap_or_else(|| panic!("the user didn't see the guest answer in the chat within {WAIT:?}"));
    eprintln!(
        "the user sees the answer as message {} from {:?} via {:?}, guest caller {:?}",
        answer.id,
        answer.from_user.as_ref().map(|u| u.id),
        answer.via_bot.as_ref().map(|u| u.id),
        answer.guest_bot_caller_user.as_ref().map(|u| u.id)
    );

    let edited = format!("edited guest answer {nonce}");
    let result = sent.edit_text(edited.as_str()).await;
    eprintln!("SentGuestMessage::edit_text returned {result:?}");
    result.expect("editing the guest answer failed");

    let update = next(&mut seen, |(was_edited, m)| *was_edited && m.id == answer.id).await;
    let update_text = update.as_ref().map(|(_, m)| m.text().to_owned());
    eprintln!("the user's edit update for message {}: {update_text:?}", answer.id);
    let fetched = user.get_messages(group, &[answer.id]).await.unwrap().into_iter().next().flatten();
    let fetched = fetched.map(|m| (m.text().to_owned(), m.edit_date));
    eprintln!("the user's fetched copy of message {} (text, edit date): {fetched:?}", answer.id);
    assert_eq!(fetched.map(|(text, _)| text).as_deref(), Some(edited.as_str()), "the user doesn't see the edit");

    if let Some(e) = untitled {
        panic!("answer_text sends an article with no title, and Telegram refused it: {e:?}");
    }
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session; the bot needs Guest Mode in @BotFather"]
async fn a_guest_answer_can_be_edited() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let (user, bot) = (kit.user.client.clone(), kit.bot.clone());
    let bot_me = bot.get_me().await.unwrap();
    if !bot_me.supports_guest_queries {
        eprintln!("skipped: the kit bot takes no guest queries; turn on Guest Mode for it in @BotFather");
        return;
    }
    let username = bot_me.username.expect("the kit bot has no username");
    // A chat the bot isn't in, so a mention of it there is a guest query.
    let Some(group) = support::test_supergroup(&user).await else { return };

    let steps = tokio::spawn(guest_steps(user.clone(), bot.clone(), group, username)).await;

    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}
