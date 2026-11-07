#[allow(dead_code)]
mod support;

use vaegram::types::{
    InlineAction, InlineKeyboardButton, InlineKeyboardMarkup, ReplyMarkup, SwitchInlineQueryChosenChat, WebAppInfo,
};

/// The bot sends inline buttons of the newer kinds to `VAEGRAM_CHAT_ID` and reads them back from the sent message.
#[tokio::test]
#[ignore = "uses a bot token and VAEGRAM_CHAT_ID"]
async fn sends_and_reads_inline_kinds() {
    let (Ok(token), Ok(chat)) = (std::env::var("VAEGRAM_BOT_TOKEN"), std::env::var("VAEGRAM_CHAT_ID")) else {
        eprintln!("skipped: set VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return;
    };
    let (api_id, api_hash) = (std::env::var("VAEGRAM_API_ID"), std::env::var("VAEGRAM_API_HASH"));
    let (Ok(api_id), Ok(api_hash)) = (api_id, api_hash) else {
        eprintln!("skipped: set VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return;
    };
    let chat: i64 = chat.parse().expect("VAEGRAM_CHAT_ID isn't a chat id");
    let bot = vaegram::Client::builder("target/vaegram-keyboards-bot", api_id.parse().unwrap(), api_hash);
    let Some(bot) = support::start_bot(bot.bot_token(token)).await else { return };
    let markup = InlineKeyboardMarkup::new([vec![
        InlineKeyboardButton::web_app("app", WebAppInfo::new("https://telegram.org")),
        InlineKeyboardButton::switch_inline_query_chosen_chat(
            "chosen",
            SwitchInlineQueryChosenChat::new("q").allow_user_chats(true).allow_group_chats(true),
        ),
    ]]);
    let sent = bot.send_message(chat, "inline kinds").reply_markup(markup.clone()).await.unwrap();
    let read = sent.reply_markup();
    bot.delete_messages(chat, &[sent.id]).await.unwrap();
    let Some(ReplyMarkup::Inline(read)) = read else { panic!("no inline markup came back") };
    let actions: Vec<_> = read.rows.iter().flatten().map(|b| b.action.clone()).collect();
    let expected: Vec<_> = markup.rows.iter().flatten().map(|b| b.action.clone()).collect();
    assert_eq!(actions, expected);
    assert!(matches!(actions[0], InlineAction::WebApp(_)));
}
