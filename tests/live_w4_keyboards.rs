#[allow(dead_code)]
mod support;

use vaegram::types::{
    InlineAction, InlineKeyboardButton, InlineKeyboardMarkup, ReplyMarkup, SwitchInlineQueryChosenChat, WebAppInfo,
};

/// The bot sends inline buttons of the newer kinds and reads them back from the sent messages. A web app button goes
/// only in a private chat with the bot (Telegram answers BUTTON_TYPE_INVALID in a group), so it goes to the kit user;
/// the chosen-chat switch goes to `VAEGRAM_CHAT_ID`.
#[tokio::test]
#[ignore = "uses a bot token, a logged-in user session and VAEGRAM_CHAT_ID"]
async fn sends_and_reads_inline_kinds() {
    let Ok(chat) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID");
        return;
    };
    let chat: i64 = chat.parse().expect("VAEGRAM_CHAT_ID isn't a chat id");
    let Some(kit) = support::Kit::from_env().await else { return };
    let me = kit.user.client.get_me().await.unwrap();
    let web_app = InlineKeyboardButton::web_app("app", WebAppInfo::new("https://telegram.org"));
    let chosen = SwitchInlineQueryChosenChat::new("q").allow_user_chats(true).allow_group_chats(true);
    let chosen = InlineKeyboardButton::switch_inline_query_chosen_chat("chosen", chosen);

    for (target, button) in [(me.id, web_app), (chat, chosen)] {
        let markup = InlineKeyboardMarkup::new([vec![button]]);
        let sent = kit.bot.send_message(target, "inline kinds").reply_markup(markup.clone()).await.unwrap();
        let read = sent.reply_markup();
        kit.bot.delete_messages(target, &[sent.id]).await.unwrap();
        let Some(ReplyMarkup::Inline(read)) = read else { panic!("no inline markup came back") };
        let actions: Vec<_> = read.rows.iter().flatten().map(|b| b.action.clone()).collect();
        let expected: Vec<_> = markup.rows.iter().flatten().map(|b| b.action.clone()).collect();
        assert_eq!(actions, expected);
        if target == me.id {
            assert!(matches!(actions[0], InlineAction::WebApp(_)));
        }
    }
}
