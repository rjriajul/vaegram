use vaegram::prelude::*;
use vaegram::types::{InlineQueryResultArticle, InputMessageContent};

fn menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new([
        vec![
            InlineKeyboardButton::callback("Yes", "yes").style(ButtonStyle::Success),
            InlineKeyboardButton::callback("No", "no").style(ButtonStyle::Danger),
        ],
        vec![InlineKeyboardButton::callback("Show an alert", "alert").style(ButtonStyle::Primary)],
        vec![
            InlineKeyboardButton::switch_inline_query_current_chat("Try inline", "vaegram"),
            InlineKeyboardButton::url("Source", "https://github.com/rjriajul/vaegram"),
        ],
    ])
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app = Client::builder("demo_bot", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .bot_token(env("VAEGRAM_BOT_TOKEN"))
        .start()
        .await?;

    app.on_message(filters::command("start"), |client, msg| async move {
        println!("start from {:?}", msg.from_user());
        msg.answer(&client, "<b>vaegram demo</b>\nTap a button:").html().reply_markup(menu()).await?;
        Ok(())
    });

    app.on_message(filters::command("keyboard"), |client, msg| async move {
        let keyboard =
            ReplyKeyboardMarkup::new([vec![KeyboardButton::new("Hello"), KeyboardButton::new("Remove keyboard")]])
                .placeholder("Pick one");
        msg.answer(&client, "Here is a reply keyboard.").reply_markup(keyboard).await?;
        Ok(())
    });

    app.on_message(filters::command("react"), |client, msg| async move {
        msg.react(&client, "👍").await?;
        Ok(())
    });

    app.on_message(
        filters::text() & filters::create(|m: &Message| m.text() == "Remove keyboard"),
        |client, msg| async move {
            msg.answer(&client, "Keyboard removed.").reply_markup(ReplyKeyboardRemove::default()).await?;
            Ok(())
        },
    );

    app.on_message(filters::text() & filters::private(), |client, msg| async move {
        println!("message: {:?}", msg.text());
        msg.reply(&client, Text::new().plain("You said: ").bold(msg.text())).await?;
        Ok(())
    });

    app.on_edited_message(filters::text(), |client, msg| async move {
        println!("edited: {:?}", msg.text());
        msg.reply(&client, format!("You edited it to: {}", msg.text())).await?;
        Ok(())
    });

    app.on_callback_query(filters::data("alert"), |client, query| async move {
        println!("alert from {}", query.from_user.first_name);
        query.answer(&client).text("This is an alert from vaegram").alert().await?;
        Ok(())
    });

    app.on_callback_query(filters::all(), |client, query| async move {
        println!("pressed {:?}", query.data);
        query.answer(&client).text(format!("You pressed {}", query.data)).await?;
        query
            .edit_message_text(&client, format!("You pressed <b>{}</b>. Tap again:", query.data))
            .html()
            .reply_markup(menu())
            .await?;
        Ok(())
    });

    app.on_inline_query(filters::all(), |client, query| async move {
        println!("inline query: {:?}", query.query);
        if query.query.is_empty() {
            return Ok(());
        }
        let results = [
            InlineQueryResultArticle::new("Plain", format!("You searched: {}", query.query)).description("plain text"),
            InlineQueryResultArticle::new("Bold", InputMessageContent::html(format!("<b>{}</b>", query.query)))
                .description("HTML"),
        ];
        query.answer(&client, results).cache_time(0).await?;
        Ok(())
    });

    app.on_message_reaction(filters::all(), |_client, update| async move {
        println!("reactions on {} changed to {:?}", update.message_id, update.new_reaction);
        Ok(())
    });

    app.on_my_chat_member(filters::all(), |_client, update| async move {
        println!("my membership in {}: {:?}", update.chat.id, update.new_chat_member.map(|m| m.status()));
        Ok(())
    });

    app.on_chat_member_updated(filters::all(), |_client, update| async move {
        println!("member update in {}: {:?}", update.chat.id, update.new_chat_member.map(|m| m.status()));
        Ok(())
    });

    println!("demo bot is running");
    app.run().await
}
