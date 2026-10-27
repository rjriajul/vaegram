use vaegram::Client;
use vaegram::raw::{base, functions};
use vaegram::session::Session;

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn creates_auth_key_and_pings() {
    let (session, _updates) = Session::connect(2, None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 42 }).await.unwrap();
    assert_eq!(pong.ping_id, 42);
}

#[tokio::test]
#[ignore = "logs in as a bot and sends a message"]
async fn bot_sends_message() {
    if std::env::var("VAEGRAM_BOT_TOKEN").is_err() {
        eprintln!("skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH, VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let app =
        Client::builder("target/vaegram-live-test", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
            .bot_token(env("VAEGRAM_BOT_TOKEN"))
            .start()
            .await
            .unwrap();

    let base::User::User(me) = app.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert!(me.bot);

    app.send_message(env("VAEGRAM_CHAT_ID").parse().unwrap(), "hello from vaegram").await.unwrap();

    let again = Client::builder("unused", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .session_string(app.export_session_string())
        .start()
        .await
        .unwrap();
    let base::User::User(me_again) = again.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert_eq!(me_again.id, me.id);
}

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn telegram_errors_come_back_as_rpc_errors() {
    match Client::builder("target/vaegram-live-bad-id", 0, "").start().await {
        Err(vaegram::Error::Rpc(e)) => assert_eq!(e.name, "CONNECTION_API_ID_INVALID"),
        other => panic!("expected CONNECTION_API_ID_INVALID, got {:?}", other.err()),
    }
}
