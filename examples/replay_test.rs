//! Tests a bot's handlers offline: record one real conversation, then replay it with no network or credentials.
//!
//! cargo run --example replay_test -- record bot.rec   # needs VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN;
//!                                                     # send the bot a private text message
//! cargo run --example replay_test -- replay bot.rec   # runs the same handlers against the recording
//!
//! A recording holds the chats and messages it saw. Only credentials are cut out, so treat it like the data itself.

use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::session::Replay;
use vaegram::storage::recording;
use vaegram::{Client, filters};

// The code under test: registered the same way live and in the replay.
fn handlers(app: &Client, replies: mpsc::UnboundedSender<String>) {
    app.on_message(filters::private() & filters::text(), move |m| {
        let replies = replies.clone();
        async move {
            let sent = m.reply(format!("pong: {}", m.text)).await?;
            let _ = replies.send(sent.text.clone());
            Ok(())
        }
    });
}

#[tokio::main]
async fn main() -> vaegram::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [mode, path] = &args[..] else { panic!("usage: replay_test record|replay <file>") };
    let (tx, mut replies) = mpsc::unbounded_channel();

    let (app, _replay) = if mode == "record" {
        let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let app = Client::builder("replay_test", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .record(path)
            .start()
            .await?;
        (app, None)
    } else {
        // Replay answers each request with what Telegram answered for that method, in order, and pushes the
        // recorded updates as the conversation reaches them.
        let replay = Replay::start(&recording::read(path.as_ref())?).await?;
        let app = Client::builder("replay_test", 1, "replay").session_string(replay.session_string()).connect().await?;
        (app, Some(replay))
    };
    handlers(&app, tx);
    let run = tokio::spawn({
        let app = app.clone();
        async move { app.run().await }
    });

    let reply = if mode == "record" {
        replies.recv().await
    } else {
        tokio::time::timeout(Duration::from_secs(10), replies.recv()).await.expect("the handler never replied")
    };
    let reply = reply.expect("the handler is gone");
    assert!(reply.starts_with("pong: "), "{reply}");
    println!("{mode}: the bot replied {reply:?}");
    app.stop();
    run.await.expect("run panicked")
}
