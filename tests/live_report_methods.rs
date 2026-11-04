#[allow(dead_code)]
mod support;

use vaegram::filters;
use vaegram::types::ReportResult;

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
