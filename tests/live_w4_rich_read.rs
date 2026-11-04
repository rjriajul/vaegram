// Run with `cargo test --test live_w4_rich_read -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the bot half of the kit
mod support;

use vaegram::types::{InputRichMessage, RichBlock, RichBlockKind, RichListItem};

#[tokio::test]
#[ignore = "uses a logged-in user session, a bot token and VAEGRAM_CHAT_ID"]
async fn rich_message_blocks_read_back() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let blocks = [
        RichBlock::heading(2, "Read back"),
        RichBlock::paragraph("one paragraph"),
        RichBlock::list([RichListItem::new("first"), RichListItem::new("second")]),
    ];
    let sent = kit.bot.send_rich_message(chat_id, InputRichMessage::blocks(blocks)).await.unwrap();
    let message = kit.bot.get_rich_message(chat_id, sent.id).await.unwrap().expect("the message exists");
    let rich = message.rich_message.expect("a rich message");
    let kinds: Vec<_> = rich.blocks().iter().map(RichBlock::kind).collect();
    assert!(matches!(kinds[0], RichBlockKind::Heading { level: 2, .. }));
    assert!(matches!(&kinds[1], RichBlockKind::Paragraph(text) if text.text() == "one paragraph"));
    assert!(matches!(&kinds[2], RichBlockKind::List(items) if items.len() == 2));
    kit.bot.delete_messages(chat_id, &[sent.id]).await.unwrap();
}
