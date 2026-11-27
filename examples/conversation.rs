//! A signup flow written as one conversation: a name, an age checked as a number, a plan picked from options and a
//! button press to confirm, with `/cancel` at any point.

use std::time::Duration;

use vaegram::prelude::*;

async fn signup(mut conv: Conversation, msg: Message) -> Result<()> {
    msg.reply("What's your name?").await?;
    let name = conv.wait_for(filters::text()).await?.text().to_owned();

    msg.reply("How old are you?").await?;
    let age: u8 =
        conv.form().number().otherwise(|m: Message| async move { m.reply("Send your age as a number.").await }).await?;

    msg.reply("Which plan: free or pro?").await?;
    let plan = conv
        .form()
        .select(["free", "pro"])
        .otherwise(|m: Message| async move { m.reply("Pick free or pro.").await })
        .await?;

    let keyboard = InlineKeyboardMarkup::new([vec![
        InlineKeyboardButton::callback("Save", "save"),
        InlineKeyboardButton::callback("Discard", "discard"),
    ]]);
    let question = msg.reply(format!("Save {name}, {age}, {plan}?")).reply_markup(keyboard).await?;
    let press =
        conv.wait_for_callback_query(filters::data("save") | filters::data("discard")).message_id(question.id).await?;
    press.answer().await?;
    if press.data == "discard" {
        msg.reply("Discarded.").await?;
        return conv.halt();
    }
    msg.reply(format!("Welcome, {name}!")).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app = Client::builder("conversation_bot", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .bot_token(env("VAEGRAM_BOT_TOKEN"))
        .start()
        .await?;

    // `/cancel` is never the conversation's, so the handler below always gets it.
    app.conversation("signup", signup).timeout(Duration::from_secs(600)).ignore(filters::command("cancel"));

    app.on_message(filters::command("start"), |msg| async move {
        if let Err(error) = msg.client().enter_conversation("signup", msg.clone()) {
            msg.reply(format!("Can't start: {error}")).await?;
        }
        Ok(())
    });
    app.on_message(filters::command("cancel"), |msg| async move {
        let ended = msg.client().exit_conversation("signup", &msg);
        msg.reply(if ended > 0 { "Cancelled." } else { "Nothing to cancel." }).await
    });
    app.run().await
}
