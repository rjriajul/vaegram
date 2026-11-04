// Run with `cargo test --test live_type_helpers -- --ignored`; see tests/support/mod.rs for the env.
// Live behaviour is unverified: these tests have not been run. Both are read-only.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore]
async fn chat_get_member_reads_me() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Some(chat_id) = std::env::var("VAEGRAM_CHAT_ID").ok().and_then(|v| v.parse::<i64>().ok()) else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID");
        return;
    };
    let user = kit.user.client;
    let me = user.get_users(["me"]).await.unwrap().remove(0);
    let chat = user.get_chat(chat_id).await.unwrap();
    let member = chat.get_member(&user, me.id).await.unwrap();
    assert_eq!(member.user.id, me.id);
}

#[tokio::test]
#[ignore]
async fn user_common_chats() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let bot = kit.bot.get_users(["me"]).await.unwrap().remove(0);
    let user = kit.user.client;
    let bot = user.get_users([bot.id]).await.unwrap().remove(0);
    bot.get_common_chats(&user).await.unwrap();
}
