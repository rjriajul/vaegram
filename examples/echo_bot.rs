use vaegram::{Client, filters};

#[tokio::main]
async fn main() -> vaegram::Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app = Client::builder("echo_bot", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .bot_token(env("VAEGRAM_BOT_TOKEN"))
        .start()
        .await?;

    app.on_message(filters::command("start"), |msg| async move {
        let name = msg.from_user.as_ref().map_or("there", |u| u.first_name.as_str());
        msg.answer(format!("Hi {name}! Send me anything and I'll send it back.")).await?;
        Ok(())
    });

    app.on_message(filters::text() & filters::private(), |msg| async move {
        msg.answer(&msg.text).await?;
        Ok(())
    });

    app.run().await
}
