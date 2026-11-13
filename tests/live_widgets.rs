// Run with `cargo test --test live_widgets -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)]
mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::filters;
use vaegram::types::{InlineMessageContent, InlineQueryResult, InlineQueryResultArticle, WidgetPublicData};

const WAIT: Duration = Duration::from_secs(30);

async fn bot_id_for_user(kit: &support::Kit) -> i64 {
    let me = kit.bot.get_me().await.unwrap();
    let username = me.username.expect("the kit bot has no username");
    kit.user.client.resolve_username(&username).await.unwrap()
}

/// No human or second account needed: `kit.bot` sends a widget to `kit.user` in private chat, `kit.user` reads
/// `Message::widget()` and polls `get_widget_public_data` (both `Modified` and `NotModified`), then `kit.bot` edits
/// public and chat data and `kit.user` receives `on_widget_data_updated`.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_sends_reads_and_updates_widget() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;

    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        if m.widget().is_some() {
            let _ = msg_tx.send(m);
        }
        async { Ok(()) }
    });
    let (upd_tx, mut upd_rx) = mpsc::unbounded_channel();
    user.on_widget_data_updated(filters::chat(bot_id), move |event| {
        let _ = upd_tx.send(event);
        async { Ok(()) }
    });

    let html = "<!doctype html><html><body><b>widget</b></body></html>";
    let ids = Arc::new(Mutex::new(Vec::new()));
    let steps = tokio::spawn({
        let bot = kit.bot.clone();
        let user = user.clone();
        let ids = ids.clone();
        async move {
            let bot_sent = bot
                .send_widget(user_id, html)
                .width(240)
                .height(120)
                .public_data("pub0")
                .chat_data("chat0")
                .allow_queries()
                .caption("widget cap")
                .await
                .unwrap();
            let user_msg = tokio::time::timeout(WAIT, msg_rx.recv())
                .await
                .expect("timed out waiting for widget message")
                .expect("widget message channel closed");
            ids.lock().unwrap().push(user_msg.id);

            let bot_widget = bot_sent.widget().expect("bot sent message has widget");
            assert_eq!(bot_widget.html, html);
            assert_eq!((bot_widget.width, bot_widget.height), (Some(240), Some(120)));
            assert_eq!(bot_widget.public_data.as_deref(), Some("pub0"));
            assert_eq!(bot_widget.chat_data.as_deref(), Some("chat0"));
            assert!(bot_widget.can_send_queries);

            let user_widget = user_msg.widget().expect("user message has widget");
            assert_eq!(user_widget.html, html);
            assert_eq!(user_widget.public_data.as_deref(), Some("pub0"));

            let polled = user.get_widget_public_data(bot_id, user_msg.id).await.unwrap();
            let WidgetPublicData::Modified { hash, data, period, .. } = polled else {
                panic!("expected Modified on initial hash=0 poll, got {polled:?}");
            };
            assert_eq!(data, "pub0");
            assert!(period >= 0);

            let unchanged = user.get_widget_public_data(bot_id, user_msg.id).hash(hash).await.unwrap();
            assert!(matches!(unchanged, WidgetPublicData::NotModified { .. }), "{unchanged:?}");

            bot.edit_message_widget_data(user_id, bot_sent.id).public_data("pub1").chat_data("chat1").await.unwrap();
            let updated = tokio::time::timeout(WAIT, upd_rx.recv())
                .await
                .expect("timed out waiting for on_widget_data_updated")
                .expect("widget data update channel closed");
            assert_eq!(updated.message_id, user_msg.id);
            assert_eq!(updated.public_data.as_deref(), Some("pub1"));
            assert_eq!(updated.chat_data.as_deref(), Some("chat1"));

            let repolled = user.get_widget_public_data(bot_id, user_msg.id).hash(hash).await.unwrap();
            assert!(
                matches!(&repolled, WidgetPublicData::Modified { data, .. } if data == "pub1"),
                "expected updated public data, got {repolled:?}"
            );
        }
    })
    .await;

    let to_delete = ids.lock().unwrap().clone();
    if !to_delete.is_empty() {
        support::cleanup(user.delete_messages(bot_id, &to_delete).await, "deleting widget message");
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: `kit.bot` replies to `/wquery` via `Message::reply_widget`, `kit.user` sends a
/// query via `request_widget_query_answer`, and `kit.bot` answers through `on_widget_query`.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_answers_widget_query() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let bot_id = bot_id_for_user(&kit).await;

    kit.bot.on_message(filters::private() & filters::command("wquery"), |m| async move {
        m.reply_widget("<div>tap</div>").allow_queries().await?;
        Ok(())
    });
    kit.bot.on_widget_query(filters::data("ping"), |q| async move {
        q.answer().data("{\"ok\":true}").await?;
        Ok(())
    });
    kit.bot.on_widget_query(filters::data("empty"), |q| async move {
        q.answer().await?;
        Ok(())
    });

    let widget_msg = kit.user.send_and_wait_reply(&kit.bot, "/wquery").await.unwrap();
    let mut to_delete = Vec::new();
    if let Some(prompt_id) = widget_msg.reply_to_message_id {
        to_delete.push(prompt_id);
    }
    to_delete.push(widget_msg.id);

    let steps = tokio::spawn({
        let user = kit.user.client.clone();
        async move {
            assert!(widget_msg.widget().is_some());

            let ans = user.request_widget_query_answer(bot_id, widget_msg.id, b"ping").await.unwrap();
            assert_eq!(ans.as_deref(), Some("{\"ok\":true}"));

            let empty = user.request_widget_query_answer(bot_id, widget_msg.id, b"empty").await.unwrap();
            assert_eq!(empty, None);
        }
    })
    .await;

    support::cleanup(kit.user.client.delete_messages(bot_id, &to_delete).await, "deleting widget query messages");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: `kit.bot` sends a shared widget (`.is_shared()`), re-sends it with
/// `send_shared_widget`, edits shared public data with `edit_shared_widget_data`, copies it with `copy_message`, and
/// tests forwarding/copy-query (`on_shared_widget_query`) if Telegram allows forwarding the widget.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_sends_and_edits_shared_widget_and_copy_flow() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;

    let (msg_tx, mut msg_rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        if m.widget().is_some() {
            let _ = msg_tx.send(m);
        }
        async { Ok(()) }
    });
    let (copy_q_tx, mut copy_q_rx) = mpsc::unbounded_channel();
    kit.bot.on_shared_widget_query(filters::data("copy_ping"), move |q| {
        let tx = copy_q_tx.clone();
        async move {
            let _ = tx.send(q.widget_id);
            q.answer().data("{\"copy\":true}").await?;
            Ok(())
        }
    });

    let ids = Arc::new(Mutex::new(Vec::new()));
    let saved_ids = Arc::new(Mutex::new(Vec::new()));
    let steps = tokio::spawn({
        let bot = kit.bot.clone();
        let user = user.clone();
        let ids = ids.clone();
        let saved_ids = saved_ids.clone();
        async move {
            let first =
                bot.send_widget(user_id, "<p>shared</p>").is_shared().allow_queries().public_data("s0").await.unwrap();
            let user_first = tokio::time::timeout(WAIT, msg_rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_first.id);
            let widget_id = first.widget().and_then(|w| w.id).expect("shared widget must carry an id");

            let second = bot.send_shared_widget(user_id, widget_id).chat_data("c2").await.unwrap();
            let user_second = tokio::time::timeout(WAIT, msg_rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_second.id);
            assert_eq!(second.widget().and_then(|w| w.id), Some(widget_id));

            bot.edit_shared_widget_data(widget_id, "s1").cache_time(5).await.unwrap();
            let polled = user.get_widget_public_data(bot_id, user_second.id).await.unwrap();
            assert!(
                matches!(&polled, WidgetPublicData::Modified { data, .. } if data == "s1"),
                "expected shared public data s1, got {polled:?}"
            );

            let copied = bot.copy_message(user_id, user_id, first.id).await.unwrap();
            let user_copied = tokio::time::timeout(WAIT, msg_rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_copied.id);
            assert_eq!(copied.widget().and_then(|w| w.id), Some(widget_id));

            // Sent widgets are protected content in production, so forwarding to Saved Messages may be refused.
            match user.forward_messages("me", bot_id, &[user_first.id]).await {
                Ok(forwarded) => {
                    let fwd = &forwarded[0];
                    saved_ids.lock().unwrap().push(fwd.id);
                    let ans = user.request_widget_query_answer("me", fwd.id, b"copy_ping").await.unwrap();
                    assert_eq!(ans.as_deref(), Some("{\"copy\":true}"));
                    let seen_id = tokio::time::timeout(WAIT, copy_q_rx.recv()).await.unwrap().unwrap();
                    assert_eq!(seen_id, widget_id);
                }
                Err(e) => {
                    eprintln!(
                        "note: forwarding widget refused as protected content ({e}); on_shared_widget_query skipped"
                    );
                }
            }
        }
    })
    .await;

    let to_delete = ids.lock().unwrap().clone();
    if !to_delete.is_empty() {
        support::cleanup(user.delete_messages(bot_id, &to_delete).await, "deleting shared widget messages");
    }
    let saved = saved_ids.lock().unwrap().clone();
    if !saved.is_empty() {
        support::cleanup(user.delete_messages("me", &saved).await, "deleting forwarded widget message");
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// No human or second account needed: verifies Telegram rejects `send_widget` from a user account.
#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_cannot_send_widget() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let err = kit.user.client.send_widget("me", "<b>nope</b>").await.unwrap_err();
    assert!(err.is("INPUT_MEDIA_WIDGET_NOT_ALLOWED"), "expected INPUT_MEDIA_WIDGET_NOT_ALLOWED, got {err:?}");
}

/// No human or second account needed (skips if the bot has inline mode disabled in @BotFather): checks Telegram's
/// production response to inline widgets (`InputMessageContent::widget`).
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token with inline mode"]
async fn inline_widget_behavior_in_production() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let bot_id = bot_id_for_user(&kit).await;

    let (tx, mut inline_err) = mpsc::unbounded_channel();
    kit.bot.on_inline_query(filters::all(), move |q| {
        let tx = tx.clone();
        async move {
            let content = InputMessageContent::widget("<b>inline</b>");
            let result = InlineQueryResult::from(InlineQueryResultArticle::new("Widget", content).id("w1"));
            let answer = q.answer([result]).cache_time(0).await;
            let _ = tx.send(answer);
            Ok(())
        }
    });

    match kit.user.client.get_inline_bot_results(bot_id, "widget").await {
        Err(e) if e.is("BOT_INLINE_DISABLED") => {
            eprintln!("skipped: turn on inline mode for the kit bot with /setinline in @BotFather");
        }
        Ok(results) => {
            let answer = tokio::time::timeout(WAIT, inline_err.recv()).await.unwrap().unwrap();
            match answer {
                Ok(()) => {
                    eprintln!("note: Telegram accepted inline widget answer ({} results)", results.results.len());
                    if let Some(first) = results.results.first() {
                        let sent = kit.user.client.send_inline_bot_result("me", results.query_id, &first.id).await;
                        eprintln!("note: send_inline_bot_result returned {sent:?}");
                        if let Ok(msg) = sent {
                            let _ = kit.user.client.delete_messages("me", &[msg.id]).await;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("note: Telegram rejected inline widget as expected in production: {e}");
                }
            }
        }
        Err(e) => {
            panic!("get_inline_bot_results failed unexpectedly: {e:?}");
        }
    }
}
