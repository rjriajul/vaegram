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
