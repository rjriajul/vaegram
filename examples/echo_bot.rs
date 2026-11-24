use vaegram::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app = Client::builder("echo_bot", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .bot_token(env("VAEGRAM_BOT_TOKEN"))
        .start()
        .await?;

    app.on_message(filters::command("start"), |client, msg| async move {
        let name = msg.from_user().map_or("there", |u| u.first_name.as_str());
        msg.answer(&client, format!("Hi {name}! Send me anything and I'll send it back.")).await?;
        Ok(())
    });

    app.on_message(filters::text() & filters::private(), |client, msg| async move {
        msg.answer(&client, msg.text()).await?;
        Ok(())
    });

    app.run().await
}
