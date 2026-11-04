#[allow(dead_code)]
mod support;

use vaegram::Client;
use vaegram::raw::base;

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn an_exported_string_logs_in_again() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let (api_id, api_hash) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let user = Client::builder(session, api_id, api_hash.clone()).connect().await.unwrap();
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("no user") };
    let exported = user.export_session_string();
    assert!(exported.starts_with("VAE_"));

    let again = Client::builder("string-login", api_id, api_hash).session_string(exported).connect().await.unwrap();
    let base::User::User(back) = again.get_me().await.unwrap() else { panic!("no user") };
    assert_eq!(back.id, me.id);
    assert_eq!(again.export_session_string(), user.export_session_string());
}
