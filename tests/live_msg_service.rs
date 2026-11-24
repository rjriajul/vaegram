mod support;

use vaegram::filters;
use vaegram::types::ServiceAction;

#[tokio::test]
#[ignore = "needs a logged-in user session and a bot"]
async fn screenshot_notification_reads_back_as_a_service_message() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        m.reply("ok").await?;
        Ok(())
    });
    let reply = kit.user.send_and_wait_reply(&kit.bot, "hi").await.unwrap();
    let user = &kit.user.client;
    user.send_screenshot_notification(reply.chat.id).await.unwrap();

    let newest = user.get_chat_history(reply.chat.id).limit(1).next().await.unwrap().expect("an empty chat");
    assert_eq!(newest.service, Some(ServiceAction::ScreenshotTaken));
}
