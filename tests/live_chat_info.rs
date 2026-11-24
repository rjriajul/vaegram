// Run with `cargo test --test live_chat_info -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary only needs the kit's clients
mod support;

use vaegram::enums::ChatMemberStatus;

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
    let bot = kit.bot.get_me().await.unwrap();
    let member = kit.user.client.get_chat_member(chat_id, bot.id).await.unwrap();
    if member.status != ChatMemberStatus::Administrator {
        eprintln!("skipped: the bot isn't an admin of VAEGRAM_CHAT_ID");
        return;
    }
    assert!(member.promoted_by.is_some_and(|u| u.id != 0));
    assert_eq!((member.chat, member.restricted_by), (None, None));
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn dialogs_have_read_positions() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let mut dialogs = kit.user.client.get_dialogs().limit(20);
    let mut read_up_to_top = 0;
    while let Some(dialog) = dialogs.next().await.unwrap() {
        assert!(dialog.unread_poll_vote_count >= 0);
        // Deleting the newest read message leaves the read mark above the top message, so those are skipped.
        let Some(top) = &dialog.top_message else { continue };
        if top.id >= dialog.last_read_inbox_message_id && dialog.last_read_inbox_message_id > 0 {
            read_up_to_top += 1;
        }
    }
    assert!(read_up_to_top > 0, "no dialog had a read mark at or below its top message");
}
