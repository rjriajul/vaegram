use vaegram::Client;
use vaegram::enums::ClientEvent;

#[tokio::main]
async fn main() -> vaegram::Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app = Client::builder("event_log", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .bot_token(env("VAEGRAM_BOT_TOKEN"))
        .start()
        .await?;

    app.on_event(|event| match event {
        ClientEvent::Reconnected { dc, attempt } => println!("back on DC {dc} after {attempt} tries"),
        ClientEvent::UpdatesDropped { count, unrecoverable, .. } => {
            println!("{count} update batches dropped, {unrecoverable} with updates that can't be fetched again")
        }
        ClientEvent::UpdatesSkipped { chat_id, from, to, reason, .. } => {
            println!("lost pts {from}..{to} of {chat_id:?}: {reason:?}")
        }
        other => println!("{other:?}"),
    });

    app.on_raw_update(|_, update| async move {
        println!("{update:?}");
        Ok(())
    });

    app.run().await
}
