// Run with `cargo test --test live_0_2_updates -- --ignored --test-threads=1`; see tests/support/mod.rs for the env.
// These tests share session files, so run with `--test-threads=1`.

#[allow(dead_code)]
mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::enums::{ChatAction, ChatMemberStatus};
use vaegram::types::{InvoiceRef, ReactionType};
use vaegram::{Client, filters};

const WAIT: Duration = Duration::from_secs(30);

fn chat_id() -> Option<i64> {
    let id = std::env::var("VAEGRAM_CHAT_ID").ok().and_then(|v| v.parse().ok());
    if id.is_none() {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a supergroup with the kit bot and user)");
    }
    id
}

async fn second_user_client() -> Option<(Client, support::Disconnect)> {
    let Ok(session) = std::env::var("VAEGRAM_SECOND_USER_SESSION") else {
        return None;
    };
    let Some(api_id) = std::env::var("VAEGRAM_API_ID").ok().and_then(|v| v.parse().ok()) else {
        eprintln!("skipped: set VAEGRAM_API_ID and VAEGRAM_API_HASH for VAEGRAM_SECOND_USER_SESSION");
        return None;
    };
    let Ok(api_hash) = std::env::var("VAEGRAM_API_HASH") else {
        eprintln!("skipped: set VAEGRAM_API_ID and VAEGRAM_API_HASH for VAEGRAM_SECOND_USER_SESSION");
        return None;
    };
    let client = Client::builder(session, api_id, api_hash).catch_up(false).connect().await.unwrap();
    let disconnect = support::Disconnect(vec![client.clone()]);
    Some((client, disconnect))
}

/// No human or second account needed: `kit.bot` reacts to `kit.user`'s message in the supergroup `VAEGRAM_CHAT_ID`,
/// and `kit.user` receives the `on_reactions` update.
#[tokio::test]
#[ignore = "uses a logged-in user session, a bot token and a supergroup VAEGRAM_CHAT_ID"]
async fn on_reactions_receives_bot_reaction() {
    let (Some(kit), Some(chat_id)) = (support::Kit::from_env().await, chat_id()) else { return };
    if chat_id > -1_000_000_000_000 {
        eprintln!("skipped: VAEGRAM_CHAT_ID must be a supergroup so both accounts share message ids");
        return;
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    kit.user.client.on_reactions(filters::chat(chat_id), move |event| {
        let _ = tx.send(event);
        async { Ok(()) }
    });

    let sent = kit.user.client.send_message(chat_id, "react for on_reactions").await.unwrap();
    let steps = tokio::spawn({
        let bot = kit.bot.clone();
        let sent_id = sent.id;
        async move {
            bot.send_reaction(chat_id, sent_id).reaction("👍").await.unwrap();
            let seen = tokio::time::timeout(WAIT, async {
                loop {
                    match rx.recv().await {
                        Some(event) if event.message_id == sent_id => return Some(event),
                        Some(_) => {}
                        None => return None,
                    }
                }
            })
            .await;
            let _ = bot.send_reaction(chat_id, sent_id).await;
            let event = seen.expect("timed out waiting for on_reactions").expect("reactions channel closed");
            assert_eq!(event.chat.id, chat_id);
            assert!(
                event.reactions.reactions.iter().any(|r| r.kind == ReactionType::Emoji("👍".into())),
                "expected 👍 in {:?}",
                event.reactions.reactions
            );
        }
    })
    .await;

    support::cleanup(kit.user.client.delete_messages(chat_id, &[sent.id]).await, "deleting reaction test message");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: pins and unpins a message in `VAEGRAM_CHAT_ID`. When the bot can pin, the bot
/// pins and `kit.user` receives `on_pinned_messages`; otherwise `kit.user` pins and `kit.bot` receives it.
#[tokio::test]
#[ignore = "uses a logged-in user session, a bot token and VAEGRAM_CHAT_ID with pin rights"]
async fn on_pinned_messages_sees_pin_and_unpin() {
    let (Some(kit), Some(chat_id)) = (support::Kit::from_env().await, chat_id()) else { return };
    if chat_id > -1_000_000_000_000 {
        eprintln!("skipped: VAEGRAM_CHAT_ID must be a supergroup so both accounts share message ids and chat_ids");
        return;
    }
    let bot_me = kit.bot.get_me().await.unwrap();
    let bot_member = kit.user.client.get_chat_member(chat_id, bot_me.id).await.unwrap();
    let bot_can_pin = bot_member.status == ChatMemberStatus::Administrator
        && bot_member.privileges.is_some_and(|p| p.can_pin_messages);

    let (actor, listener) = if bot_can_pin { (&kit.bot, &kit.user.client) } else { (&kit.user.client, &kit.bot) };
    let actor_label = if bot_can_pin { "bot" } else { "user" };
    let listener_label = if bot_can_pin { "user" } else { "bot" };

    let (tx, mut rx) = mpsc::unbounded_channel();
    let listener_events = Arc::new(Mutex::new(Vec::new()));
    let listener_client_events = Arc::new(Mutex::new(Vec::new()));

    listener.on_event({
        let events = listener_client_events.clone();
        move |e| events.lock().unwrap().push(format!("{e:?}"))
    });
    listener.on_pinned_messages(filters::all(), {
        let listener_events = listener_events.clone();
        move |event| {
            listener_events.lock().unwrap().push((event.chat.id, event.is_pinned, event.message_ids.clone()));
            if event.chat.id == chat_id {
                let _ = tx.send((event.is_pinned, event.message_ids));
            }
            async { Ok(()) }
        }
    });

    async fn wait_pinned(
        listener: &Client,
        chat_id: i64,
        rx: &mut mpsc::UnboundedReceiver<(bool, Vec<i32>)>,
        want_pinned: bool,
        sent_id: i32,
        allow_recovery: bool,
    ) -> Result<&'static str, &'static str> {
        // What this proves: `on_pinned_messages` delivers both pin and unpin for the supergroup. Because Telegram may
        // withhold message-less channel updates from push, live push is only exercised when Telegram sends the update
        // during the initial 6 s push-only window; a user listener polls `recover_gaps` (`getChannelDifference`) after
        // that, whereas a bot listener must receive both updates by push alone.
        let deadline = tokio::time::sleep(WAIT);
        tokio::pin!(deadline);
        let push_only = tokio::time::sleep(Duration::from_secs(6));
        tokio::pin!(push_only);
        let mut poll = tokio::time::interval(Duration::from_secs(2));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut recovered = false;

        loop {
            if !allow_recovery || !recovered {
                tokio::select! {
                    biased;
                    msg = rx.recv() => match msg {
                        Some((is_pinned, ids)) if is_pinned == want_pinned && ids.contains(&sent_id) => {
                            return Ok(if recovered { "recovery" } else { "push" });
                        }
                        Some(_) => {}
                        None => return Err("pinned channel closed"),
                    },
                    _ = &mut push_only, if allow_recovery && !recovered => {
                        recovered = true;
                        let _ = listener.recover_gaps().chat_ids([chat_id]).await;
                        poll.reset();
                    }
                    _ = &mut deadline => return Err("timed out"),
                }
            } else {
                tokio::select! {
                    biased;
                    msg = rx.recv() => match msg {
                        Some((is_pinned, ids)) if is_pinned == want_pinned && ids.contains(&sent_id) => {
                            return Ok("recovery");
                        }
                        Some(_) => {}
                        None => return Err("pinned channel closed"),
                    },
                    _ = poll.tick() => {
                        let _ = listener.recover_gaps().chat_ids([chat_id]).await;
                    }
                    _ = &mut deadline => return Err("timed out"),
                }
            }
        }
    }

    let sent = actor.send_message(chat_id, "pin for on_pinned_messages").await.unwrap();
    let steps = tokio::spawn({
        let actor = actor.clone();
        let listener = listener.clone();
        let sent_id = sent.id;
        async move {
            let diag = || {
                format!(
                    "bot_can_pin={bot_can_pin} (actor={actor_label}, listener={listener_label}, sent_id={sent_id}, \
                     listener_events={:?}, listener_client_events={:?})",
                    *listener_events.lock().unwrap(),
                    *listener_client_events.lock().unwrap(),
                )
            };
            if let Err(e) = actor.pin_chat_message(chat_id, sent_id).disable_notification().await {
                assert!(support::flood_limited(&e, "pinning a message"), "{e:?}");
                return;
            }
            let pin_via = match wait_pinned(&listener, chat_id, &mut rx, true, sent_id, bot_can_pin).await {
                Ok(via) => via,
                Err(why) => panic!("pin never reached on_pinned_messages: {why}; {}", diag()),
            };
            if let Err(e) = actor.unpin_chat_message(chat_id, sent_id).await {
                assert!(support::flood_limited(&e, "unpinning a message"), "{e:?}");
                return;
            }
            let unpin_via = match wait_pinned(&listener, chat_id, &mut rx, false, sent_id, bot_can_pin).await {
                Ok(via) => via,
                Err(why) => {
                    panic!("unpin never reached on_pinned_messages (pin via {pin_via}): {why}; {}", diag());
                }
            };
            eprintln!("on_pinned_messages: pin via {pin_via}, unpin via {unpin_via}");
        }
    })
    .await;

    support::cleanup(actor.delete_messages(chat_id, &[sent.id]).await, "deleting pin test message");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// Requires a second user account (`VAEGRAM_SECOND_USER_SESSION`) or human action (`VAEGRAM_HUMAN_READ=1`): bots do
/// not send or receive read receipts, so another user session must read the message sent by `VAEGRAM_USER_SESSION`
/// (or, with `VAEGRAM_HUMAN_READ=1`, a human must open Saved Messages / the bot chat on another device within 30 s).
#[tokio::test]
#[ignore = "needs VAEGRAM_SECOND_USER_SESSION (or VAEGRAM_HUMAN_READ=1 and another device reading a message)"]
async fn on_read_history_sees_peer_read() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let second = second_user_client().await;
    let human = std::env::var("VAEGRAM_HUMAN_READ").is_ok_and(|v| v == "1");
    if second.is_none() && !human {
        eprintln!(
            "skipped: set VAEGRAM_SECOND_USER_SESSION (or VAEGRAM_HUMAN_READ=1 and read a message on another device)"
        );
        return;
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    kit.user.client.on_read_history(filters::all(), move |event| {
        let _ = tx.send(event);
        async { Ok(()) }
    });

    if let Some((second, _disconnect)) = second {
        let user_me = kit.user.client.get_me().await.unwrap();
        let second_me = second.get_me().await.unwrap();
        let peer_on_user = match second_me.username.as_deref() {
            Some(username) => kit.user.client.resolve_username(username).await.unwrap(),
            None => second_me.id,
        };
        let peer_on_second = match user_me.username.as_deref() {
            Some(username) => second.resolve_username(username).await.unwrap(),
            None => user_me.id,
        };
        let sent = kit.user.client.send_message(peer_on_user, "read receipt check").await.unwrap();
        let steps = tokio::spawn(async move {
            second.read_chat_history(peer_on_second, 0).await.unwrap();
            let seen = tokio::time::timeout(WAIT, async {
                loop {
                    match rx.recv().await {
                        Some(event) if event.chat.id == peer_on_user && event.is_outgoing => return Some(event),
                        Some(_) => {}
                        None => return None,
                    }
                }
            })
            .await;
            let event = seen.expect("timed out waiting for on_read_history").expect("read channel closed");
            assert!(event.max_id >= sent.id, "expected max_id >= {}, got {}", sent.id, event.max_id);
            drop(_disconnect);
        })
        .await;
        support::cleanup(kit.user.client.delete_messages(peer_on_user, &[sent.id]).await, "deleting read check msg");
        if let Err(failed) = steps {
            std::panic::resume_unwind(failed.into_panic());
        }
    } else {
        let sent = kit.user.client.send_message("me", "open/read another chat on a second device now").await.unwrap();
        eprintln!("action: open/read an unread incoming chat for this account on another device within {WAIT:?}");
        let steps = tokio::spawn(async move {
            let event = tokio::time::timeout(WAIT, rx.recv())
                .await
                .expect("timed out waiting for human read receipt")
                .expect("read channel closed");
            assert!(event.max_id > 0);
        })
        .await;
        support::cleanup(kit.user.client.delete_messages("me", &[sent.id]).await, "deleting human prompt");
        if let Err(failed) = steps {
            std::panic::resume_unwind(failed.into_panic());
        }
    }
}

/// Requires a second user account (`VAEGRAM_SECOND_USER_SESSION`) or human action (`VAEGRAM_HUMAN_JOIN_REQUEST=1`):
/// bots cannot join chats through invite links, and a chat owner cannot request to join their own group. With
/// `VAEGRAM_SECOND_USER_SESSION`, the second account requests to join the temporary supergroup; with
/// `VAEGRAM_HUMAN_JOIN_REQUEST=1`, a human must open the printed invite link and tap Request to Join within 30 s.
#[tokio::test]
#[ignore = "needs VAEGRAM_SECOND_USER_SESSION (or VAEGRAM_HUMAN_JOIN_REQUEST=1); creates and deletes a supergroup"]
async fn on_pending_join_requests_sees_request() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let second = second_user_client().await;
    let human = std::env::var("VAEGRAM_HUMAN_JOIN_REQUEST").is_ok_and(|v| v == "1");
    if second.is_none() && !human {
        eprintln!("skipped: set VAEGRAM_SECOND_USER_SESSION (or VAEGRAM_HUMAN_JOIN_REQUEST=1)");
        return;
    }
    let user = kit.user.client.clone();
    let Some(group) = support::test_supergroup(&user).await else { return };

    let steps = tokio::spawn({
        let user = user.clone();
        async move {
            let (tx, mut rx) = mpsc::unbounded_channel();
            user.on_pending_join_requests(filters::chat(group), move |event| {
                let _ = tx.send(event);
                async { Ok(()) }
            });

            let link = user.create_chat_invite_link(group).creates_join_request().await.unwrap();
            if let Some((second, _disconnect)) = second {
                let second_id = second.get_me().await.unwrap().id;
                match second.join_chat(link.invite_link.as_str()).await {
                    Err(e) if e.is("INVITE_REQUEST_SENT") => {}
                    other => panic!("expected INVITE_REQUEST_SENT, got {other:?}"),
                }
                let event = tokio::time::timeout(WAIT, rx.recv())
                    .await
                    .expect("timed out waiting for on_pending_join_requests")
                    .expect("join requests channel closed");
                assert_eq!(event.chat.id, group);
                assert!(event.pending_count >= 1);
                assert!(event.recent_requesters.iter().any(|u| u.id == second_id));
            } else {
                eprintln!("action: open {} and tap Request to Join within {WAIT:?}", link.invite_link);
                let event = tokio::time::timeout(WAIT, rx.recv())
                    .await
                    .expect("timed out waiting for human join request")
                    .expect("join requests channel closed");
                assert_eq!(event.chat.id, group);
                assert!(event.pending_count >= 1);
            }
        }
    })
    .await;

    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: `kit.bot` sends `ChatAction::Typing` to `kit.user`, and `kit.user` receives
/// `on_chat_action`.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn on_chat_action_sees_bot_typing() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let me = kit.user.client.get_me().await.unwrap();
    let bot_id = kit.bot.get_me().await.unwrap().id;

    let (tx, mut rx) = mpsc::unbounded_channel();
    kit.user.client.on_chat_action(filters::chat(bot_id), move |event| {
        let _ = tx.send(event);
        async { Ok(()) }
    });

    kit.bot.send_chat_action(me.id, ChatAction::Typing).await.unwrap();
    let seen = tokio::time::timeout(WAIT, async {
        loop {
            match rx.recv().await {
                Some(event) if event.action == ChatAction::Typing => return Some(event),
                Some(_) => {}
                None => return None,
            }
        }
    })
    .await;
    let _ = kit.bot.send_chat_action(me.id, ChatAction::Cancel).await;

    let event = seen.expect("timed out waiting for on_chat_action").expect("chat action channel closed");
    assert_eq!(event.chat.id, bot_id);
    assert_eq!(event.from_user.as_ref().map(|u| u.id), Some(bot_id));
}

/// Requires real Telegram Stars on `VAEGRAM_USER_SESSION` (`VAEGRAM_SPEND_STARS=1` and `VAEGRAM_STARS_INVOICE`),
/// because `UpdateStarsBalance` fires only when the account's Stars balance actually changes.
#[tokio::test]
#[ignore = "spends real Stars: needs VAEGRAM_SPEND_STARS=1 and VAEGRAM_STARS_INVOICE"]
async fn on_stars_balance_sees_spend() {
    if std::env::var("VAEGRAM_SPEND_STARS").as_deref() != Ok("1") {
        eprintln!("skipped: set VAEGRAM_SPEND_STARS=1 to let this test spend real Stars");
        return;
    }
    let Ok(link) = std::env::var("VAEGRAM_STARS_INVOICE") else {
        eprintln!("skipped: set VAEGRAM_STARS_INVOICE (a Stars invoice link)");
        return;
    };
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;

    let before = user.get_my_star_balance().await.unwrap();
    let form = user.get_payment_form(InvoiceRef::Link(link)).await.unwrap();
    if form.currency != "XTR" {
        eprintln!("skipped: VAEGRAM_STARS_INVOICE currency is {}, expected XTR", form.currency);
        return;
    }
    if before.amount < form.total_amount {
        eprintln!("skipped: balance ({}) is below invoice total ({})", before.amount, form.total_amount);
        return;
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_stars_balance(filters::all(), move |event| {
        let _ = tx.send(event);
        async { Ok(()) }
    });

    user.send_payment_form(form).await.unwrap();
    let event = tokio::time::timeout(WAIT, rx.recv())
        .await
        .expect("timed out waiting for on_stars_balance")
        .expect("stars balance channel closed");
    assert!(event.balance.amount < before.amount, "expected balance < {}, got {}", before.amount, event.balance.amount);
}
