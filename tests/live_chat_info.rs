// Run with `cargo test --test live_chat_info -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary only needs the kit's clients
mod support;

use vaegram::enums::ChatMemberStatus;
use vaegram::raw::base;

fn chat_id() -> Option<i64> {
    let id = std::env::var("VAEGRAM_CHAT_ID").ok().and_then(|v| v.parse().ok());
    if id.is_none() {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a group the bot is in)");
    }
    id
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn bot_member_shows_who_promoted_it() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Some(chat_id) = chat_id() else { return };
    let base::User::User(bot) = kit.bot.get_me().await.unwrap() else { panic!("the bot has no user") };
    let member = kit.user.client.get_chat_member(chat_id, bot.id).await.unwrap();
    if member.status != ChatMemberStatus::Administrator {
        eprintln!("skipped: the bot isn't an admin of VAEGRAM_CHAT_ID");
        return;
    }
    assert!(member.promoted_by.is_some_and(|u| u.id != 0));
    assert_eq!((member.chat, member.restricted_by), (None, None));
}
