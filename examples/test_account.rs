//! Logs a real number into Telegram's test servers and saves it to `test_account.session`.
//!
//! `test_account PHONE` sends the code. `test_account PHONE CODE` signs in with it.

use vaegram::{Client, Error};

#[tokio::main]
async fn main() -> vaegram::Result<()> {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let phone = args.first().expect("usage: test_account PHONE [CODE]");
    let app = Client::builder("test_account", env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .test_mode()
        .connect()
        .await?;
    let hash_file = "test_account.session.hash.tmp";

    let Some(code) = args.get(1) else {
        let sent = app.send_code(phone).await?;
        std::fs::write(hash_file, &sent.phone_code_hash).unwrap();
        println!("Code sent ({:?}). Run again with the code.", sent.kind);
        return Ok(());
    };
    let hash = std::fs::read_to_string(hash_file).expect("send the code first");
    let me = match app.sign_in(phone, &hash, code).await {
        Err(Error::SignUpRequired) => app.sign_up(phone, &hash, "Test", "User").await?,
        other => other?,
    };
    let _ = std::fs::remove_file(hash_file);
    println!("Logged in on the test server: {} (id {})", me.first_name, me.id);
    Ok(())
}
