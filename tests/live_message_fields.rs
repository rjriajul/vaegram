// Run with `cargo test --test live_message_fields -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the bot half of the kit
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session, a bot token and VAEGRAM_CHAT_ID"]
async fn sent_message_has_no_join_type_or_import() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let sent = kit.bot.send_message(chat_id, "message fields").await.unwrap();
    assert!(sent.chat_join_type.is_none());
    assert!(sent.forward_origin.is_none());
    assert!(sent.reply_to_story().await.unwrap().is_none());
    kit.bot.delete_messages(chat_id, &[sent.id]).await.unwrap();
}
