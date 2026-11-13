// Run with `cargo test --test live_0_2_methods -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)]
mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::enums::ChatMemberStatus;
use vaegram::filters;
use vaegram::types::{KeyboardButton, KeyboardButtonRequestUsers, Message, ReplyKeyboardMarkup, ServiceAction};

const WAIT: Duration = Duration::from_secs(30);

async fn bot_id_for_user(kit: &support::Kit) -> i64 {
    let me = kit.bot.get_me().await.unwrap();
    let username = me.username.expect("the kit bot has no username");
    kit.user.client.resolve_username(&username).await.unwrap()
}

/// Requires Telegram Premium on `VAEGRAM_USER_SESSION` (quick replies are a Telegram Business feature). No human or
/// second user account is needed.
#[tokio::test]
#[ignore = "uses a logged-in user session with Telegram Premium and a bot token"]
async fn quick_replies_round_trip() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    if !user.get_me().await.unwrap().is_premium {
        eprintln!("skipped: quick replies need Telegram Premium on the VAEGRAM_USER_SESSION account");
        return;
    }

    let bot_id = bot_id_for_user(&kit).await;
    let existing = user.get_quick_replies().await.unwrap();
    let mut name = String::from("vaegram_live_qr");
    while existing.iter().any(|q| q.shortcut == name) {
        name.push('x');
    }
    assert!(user.check_quick_reply_shortcut(name.as_str()).await.unwrap());

    let added = match user.add_quick_reply_message(name.as_str(), "first").await {
        Ok(msg) => msg,
        Err(e) if support::flood_limited(&e, "adding a quick reply") => return,
        Err(e) if e.is("QUICK_REPLIES_TOO_MUCH") => {
            eprintln!("skipped: the account reached Telegram's quick reply shortcut limit");
            return;
        }
        Err(e) => panic!("add_quick_reply_message failed: {e:?}"),
    };
    let shortcut_id = added.quick_reply_shortcut_id().expect("quick_reply_shortcut_id");
    let sent_to_bot = Arc::new(AtomicBool::new(false));

    let steps = tokio::spawn({
        let user = user.clone();
        let bot = kit.bot.clone();
        let existing_ids: Vec<i32> = existing.iter().map(|q| q.shortcut_id).collect();
        let sent_to_bot = sent_to_bot.clone();
        async move {
            let second = user.add_quick_reply_message(name.as_str(), "second").await.unwrap();
            let listed = user.get_quick_reply_messages(shortcut_id).await.unwrap();
            assert_eq!(listed.len(), 2);

            let filtered = user.get_quick_reply_messages(shortcut_id).message_ids([added.id]).await.unwrap();
            assert_eq!(filtered.len(), 1);
            assert_eq!(filtered[0].text(), "first");

            let edited = user.edit_quick_reply_message(shortcut_id, added.id, "edited").await.unwrap();
            assert_eq!(edited.text(), "edited");

            let renamed = format!("{name}_r");
            user.edit_quick_reply_shortcut(shortcut_id, renamed.as_str()).await.unwrap();
            let replies = user.get_quick_replies().await.unwrap();
            assert!(replies.iter().any(|q| q.shortcut_id == shortcut_id && q.shortcut == renamed));

            if !existing_ids.is_empty() {
                let mut reordered = vec![shortcut_id];
                reordered.extend(existing_ids.iter().copied());
                user.reorder_quick_replies(reordered).await.unwrap();
            }

            let (tx, mut rx) = mpsc::unbounded_channel();
            bot.on_message(filters::private() & filters::text(), move |m| {
                let _ = tx.send(m.text().to_owned());
                async { Ok(()) }
            });
            sent_to_bot.store(true, Ordering::Relaxed);
            user.send_quick_reply_messages(bot_id, shortcut_id, &[added.id]).await.unwrap();
            let delivered = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
            assert_eq!(delivered, "edited");

            user.delete_quick_reply_messages(shortcut_id, &[second.id]).await.unwrap();
            let remaining = user.get_quick_reply_messages(shortcut_id).await.unwrap();
            assert_eq!(remaining.len(), 1);
        }
    })
    .await;

    support::cleanup(user.delete_quick_reply_shortcut(shortcut_id).await, "deleting quick reply shortcut");
    if sent_to_bot.load(Ordering::Relaxed) {
        if let Ok(history) = user.get_chat_history(bot_id).limit(5).await {
            let sent_ids: Vec<i32> =
                history.into_iter().filter(|m| m.outgoing && m.text() == "edited").map(|m| m.id).collect();
            if !sent_ids.is_empty() {
                support::cleanup(user.delete_messages(bot_id, &sent_ids).await, "deleting sent quick reply message");
            }
        }
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: creates a temporary supergroup, adds `kit.bot`, removes it with
/// `remove_chat_member`, checks that it is `Left` (not banned) and can be added back, then deletes the supergroup.
#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session and a bot"]
async fn remove_chat_member_lets_member_rejoin() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = kit.user.client.clone();
    let bot_id = bot_id_for_user(&kit).await;
    let Some(group) = support::test_supergroup(&user).await else { return };

    let steps = tokio::spawn({
        let user = user.clone();
        async move {
            match user.add_chat_members(group, [bot_id], 0).await {
                Ok(failed) => assert!(failed.is_empty(), "could not add kit bot: {failed:?}"),
                Err(e) if e.is("BOT_GROUPS_BLOCKED") || e.is("USER_BOT_NO_GROUPS") => {
                    eprintln!("skipped: the kit bot has group joining disabled in @BotFather");
                    return;
                }
                Err(e) => panic!("add_chat_members failed: {e:?}"),
            }
            assert!(user.get_chat_member(group, bot_id).await.unwrap().is_member);

            user.remove_chat_member(group, bot_id).await.unwrap();
            let after = user.get_chat_member(group, bot_id).await.unwrap();
            assert!(!after.is_member);
            assert_eq!(after.status, ChatMemberStatus::Left);

            let readded = user.add_chat_members(group, [bot_id], 0).await.unwrap();
            assert!(readded.is_empty(), "removed member could not rejoin: {readded:?}");
            assert!(user.get_chat_member(group, bot_id).await.unwrap().is_member);
        }
    })
    .await;

    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: creates a temporary basic group with `kit.bot` (because only basic groups use
/// `revoke_messages`), sends a message, bans `kit.bot` with `.revoke_messages(true)`, checks removal, and leaves.
#[tokio::test]
#[ignore = "creates and leaves a basic group with a logged-in user session and a bot"]
async fn ban_chat_member_revoke_messages_in_basic_group() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = kit.user.client.clone();
    let bot_id = bot_id_for_user(&kit).await;

    let created = match user.create_group("vaegram live test", [bot_id]).await {
        Ok(created) => created,
        Err(e) if support::flood_limited(&e, "creating a basic group") => return,
        Err(e) if e.is("BOT_GROUPS_BLOCKED") || e.is("USER_BOT_NO_GROUPS") => {
            eprintln!("skipped: the kit bot has group joining disabled in @BotFather");
            return;
        }
        Err(e) => panic!("create_group failed: {e:?}"),
    };
    if !created.failed_members.is_empty() {
        support::cleanup(user.leave_chat(created.chat.id).await, "leaving temporary basic group");
        eprintln!("skipped: could not add kit bot to basic group: {:?}", created.failed_members);
        return;
    }
    let group_id = created.chat.id;

    let steps = tokio::spawn({
        let user = user.clone();
        let bot = kit.bot.clone();
        async move {
            let sent = user.send_message(group_id, "history before revoke").await.unwrap();
            user.ban_chat_member(group_id, bot_id).revoke_messages(true).await.unwrap();

            let member = user.get_chat_member(group_id, bot_id).await.unwrap();
            assert!(!member.is_member);
            // After revoking, the bot can no longer read the group's messages.
            let fetched = bot.get_messages(group_id, &[sent.id]).await;
            assert!(
                match &fetched {
                    Err(_) => true,
                    Ok(msgs) => msgs.first().is_none_or(Option::is_none),
                },
                "bot still saw revoked basic-group message: {fetched:?}"
            );
        }
    })
    .await;

    support::cleanup(user.leave_chat(group_id).await, "leaving temporary basic group");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: `kit.bot` sends a reply keyboard with `request_users`, and `kit.user` answers
/// it via `send_bot_requested_peer`.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn send_bot_requested_peer_shares_user() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let (tx, mut bot_seen) = mpsc::unbounded_channel();

    kit.bot.on_message(filters::private() & filters::command("reqpeer"), |m| async move {
        let button = KeyboardButton::new("Pick user").request_users(KeyboardButtonRequestUsers::new(77));
        let markup = ReplyKeyboardMarkup::new([vec![button]]).one_time(true);
        m.reply("pick one").reply_markup(markup).await?;
        Ok(())
    });
    kit.bot.on_message(
        filters::private() & filters::create(|m: &Message| matches!(m.service, Some(ServiceAction::UsersShared(_)))),
        move |m| {
            if let Some(ServiceAction::UsersShared(shared)) = m.service.clone() {
                let _ = tx.send(shared);
            }
            async { Ok(()) }
        },
    );

    let asked = kit.user.send_and_wait_reply(&kit.bot, "/reqpeer").await.unwrap();
    let bot_id = asked.chat.id;
    let ids = Arc::new(Mutex::new(Vec::new()));
    {
        let mut guard = ids.lock().unwrap();
        if let Some(prompt_id) = asked.reply_to_message_id {
            guard.push(prompt_id);
        }
        guard.push(asked.id);
    }

    let steps = tokio::spawn({
        let user = kit.user.client.clone();
        let ids = ids.clone();
        async move {
            let me = user.get_me().await.unwrap();
            let shared_msg = user.send_bot_requested_peer(bot_id, asked.id, 77, [me.id]).await.unwrap();
            ids.lock().unwrap().push(shared_msg.id);

            let Some(ServiceAction::UsersShared(user_copy)) = shared_msg.service.clone() else {
                panic!("expected ServiceAction::UsersShared, got {:?}", shared_msg.service);
            };
            assert_eq!(user_copy.request_id, 77);
            assert!(user_copy.users.iter().any(|u| u.user_id == me.id));

            let bot_copy = tokio::time::timeout(WAIT, bot_seen.recv())
                .await
                .expect("timed out waiting for bot UsersShared service message")
                .expect("shared channel closed");
            assert_eq!(bot_copy.request_id, 77);
            assert!(bot_copy.users.iter().any(|u| u.user_id == me.id));
        }
    })
    .await;

    let to_delete = ids.lock().unwrap().clone();
    support::cleanup(kit.user.client.delete_messages(bot_id, &to_delete).await, "deleting request-peer messages");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: creates a temporary supergroup, sends a regular poll with
/// `.allows_revoting(false)` (checking `retract_vote` fails) and a quiz with `.allows_revoting(true)`, then cleans up.
#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session"]
async fn poll_allows_revoting_controls_retraction() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = kit.user.client.clone();
    let Some(group) = support::test_supergroup(&user).await else { return };

    let steps = tokio::spawn({
        let user = user.clone();
        async move {
            let fixed = user.send_poll(group, "Fixed?", ["A", "B"]).allows_revoting(false).await.unwrap();
            fixed.vote(&[0]).await.unwrap();
            let err = fixed.retract_vote().await.unwrap_err();
            assert!(err.rpc().is_some(), "expected Telegram RPC error retracting non-revotable poll: {err:?}");

            // Quizzes disable revoting by default; `.allows_revoting(true)` asks Telegram to allow retracting.
            let quiz = user.send_poll(group, "Quiz?", ["1", "2"]).quiz([0]).allows_revoting(true).await.unwrap();
            quiz.vote(&[0]).await.unwrap();
            quiz.retract_vote().await.unwrap();
        }
    })
    .await;

    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}
