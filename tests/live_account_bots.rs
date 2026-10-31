#[allow(dead_code)]
mod support;

use vaegram::raw::base;
use vaegram::{Client, Error};

fn api() -> (i32, String) {
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
}

async fn user_client() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let (api_id, api_hash) = api();
    Some(Client::builder(session, api_id, api_hash).connect().await.unwrap())
}

fn rpc_is(error: &Error, name: &str) -> bool {
    error.rpc().is_some_and(|e| e.is(name))
}

/// A test-server number, `99966` + DC 2 + four random digits; its login code is the DC digit five times.
fn test_number() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    format!("999662{:04}", nanos % 10_000)
}

#[tokio::test]
#[ignore = "signs up a throwaway account on Telegram's test servers"]
async fn test_account_logs_in_with_a_resent_code() {
    if std::env::var("VAEGRAM_API_ID").is_err() {
        eprintln!("skipped: set VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return;
    }
    let (api_id, api_hash) = api();
    let session = "target/vaegram-live-test-login";
    let client = Client::builder(session, api_id, api_hash).test_mode().connect().await.unwrap();
    assert!(!client.is_authorized().await.unwrap());

    let phone = test_number();
    let mut hash = client.send_code(&phone).await.unwrap().phone_code_hash;
    match client.resend_code(&phone, &hash).await {
        Ok(code) => hash = code.phone_code_hash,
        // Test numbers may have no second delivery method; the first hash stays valid then.
        Err(e) if rpc_is(&e, "SEND_CODE_UNAVAILABLE") => {}
        Err(e) => panic!("{e}"),
    }
    match client.sign_in(&phone, &hash, "22222").await {
        Ok(_) => {}
        Err(Error::SignUpRequired) => {
            client.sign_up(&phone, &hash, "Live", "Test").await.unwrap();
        }
        Err(e) => panic!("{e}"),
    }

    assert!(client.is_authorized().await.unwrap());
    // A fresh test account has no cloud password.
    assert_eq!(client.get_password_hint().await.unwrap(), None);

    let me = client.get_me().await.unwrap();
    let new_phone = test_number();
    match client.send_change_phone_code(&new_phone).await {
        Ok(code) => {
            let user = client.change_phone_number(&new_phone, &code.phone_code_hash, "22222").await.unwrap();
            let base::User::User(me) = me else { panic!("no account") };
            assert_eq!(user.id, me.id);
        }
        // The account was created a moment ago, and Telegram holds number changes back for about a day.
        Err(e) if rpc_is(&e, "FRESH_CHANGE_PHONE_FORBIDDEN") => {}
        Err(e) => panic!("{e}"),
    }
    client.log_out().await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_session_is_authorized() {
    let Some(user) = user_client().await else { return };
    assert!(user.is_authorized().await.unwrap());
}
