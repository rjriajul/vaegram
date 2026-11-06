#[allow(dead_code)]
mod support;

use std::time::Duration;

use vaegram::filters;
use vaegram::types::{ReportResult, Text};

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn report_lists_reasons_without_reporting() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        m.reply("ok").await?;
        Ok(())
    });
    let reply = kit.user.send_and_wait_reply(&kit.bot, "hi").await.unwrap();
    let user = &kit.user.client;
    // The first step only lists the reasons: no option is ever sent, so nothing is reported.
    let result = user.report_messages(reply.chat.id, &[reply.id]).await.unwrap();
    user.delete_messages(reply.chat.id, &[reply.id]).await.unwrap();
    assert!(matches!(&result, ReportResult::ChooseOption { options, .. } if !options.is_empty()), "{result:?}");
}

fn chat_id() -> Option<i64> {
    let id = std::env::var("VAEGRAM_CHAT_ID").ok().and_then(|v| v.parse().ok());
    if id.is_none() {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a group with the kit bot and user)");
    }
    id
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token; posts a live location in VAEGRAM_CHAT_ID"]
async fn recent_locations_include_a_live_location() {
    let (Some(kit), Some(chat_id)) = (support::Kit::from_env().await, chat_id()) else { return };
    let user = &kit.user.client;
    let sent = user.send_location(chat_id, 52.52, 13.405).live_period(60).await.unwrap();
    let recent = user.get_recent_locations(chat_id, 10).await;
    user.stop_message_live_location(chat_id, sent.id).await.unwrap();
    user.delete_messages(chat_id, &[sent.id]).await.unwrap();
    assert!(recent.unwrap().iter().any(|m| m.id == sent.id));
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token; the bot mentions and reacts in VAEGRAM_CHAT_ID"]
async fn unread_mentions_and_reactions_from_the_bot() {
    let (Some(kit), Some(chat_id)) = (support::Kit::from_env().await, chat_id()) else { return };
    let user = &kit.user.client;
    let me = user.get_me().await.unwrap();
    let mine = user.send_message(chat_id, "react to this").await.unwrap();
    let mention = kit.bot.send_message(chat_id, Text::new().mention("you", me.id)).await.unwrap();
    // Only supergroups share message ids between accounts, so only there can the bot react to the user's message.
    let shared_ids = chat_id < -1_000_000_000_000;
    if shared_ids {
        kit.bot.send_reaction(chat_id, mine.id).reaction("👍").await.unwrap();
    }
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut mentions = Vec::new();
    let mut unread = user.get_unread_mentions(chat_id).limit(20);
    while let Some(m) = unread.next().await.unwrap() {
        mentions.push(m.text().to_owned());
    }
    let mut reacted = Vec::new();
    let mut unread = user.get_unread_reactions(chat_id).limit(20);
    while let Some(m) = unread.next().await.unwrap() {
        reacted.push(m.id);
    }
    user.read_mentions(chat_id).await.unwrap();
    user.read_reactions(chat_id).await.unwrap();
    kit.bot.delete_messages(chat_id, &[mention.id]).await.unwrap();
    user.delete_messages(chat_id, &[mine.id]).await.unwrap();

    assert!(mentions.iter().any(|text| text == "you"), "{mentions:?}");
    assert!(!shared_ids || reacted.contains(&mine.id), "{reacted:?}");
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn attached_recent_stickers_and_old_featured_load() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    user.get_recent_stickers().attached().await.unwrap();
    let current = user.get_featured_sticker_sets().await.unwrap();
    let older = user.get_featured_sticker_sets().offset(current.len() as i32).limit(5).await.unwrap();
    assert!(older.len() <= 5, "{}", older.len());
}
