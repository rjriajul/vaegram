use vaegram::Client;
use vaegram::enums::PhoneNumberCodeType;

/// Asks for a change-number code to the logged-in account's own number. The code is never used, so the number
/// doesn't change. Telegram refuses it for about a day after a login, which also counts as the request working.
#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn change_kind_sends_a_code() {
    let vars = ["VAEGRAM_USER_SESSION", "VAEGRAM_API_ID", "VAEGRAM_API_HASH"].map(std::env::var);
    let [Ok(session), Ok(api_id), Ok(api_hash)] = vars else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION, VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return;
    };
    let client = Client::builder(session, api_id.parse().unwrap(), api_hash).connect().await.unwrap();
    let vaegram::raw::base::User::User(me) = client.get_me().await.unwrap() else { panic!("no account") };
    let phone = me.phone.expect("the account has no phone number");
    match client.send_phone_number_code(phone).kind(PhoneNumberCodeType::Change).await {
        Ok(code) => assert!(!code.phone_code_hash.is_empty()),
        Err(e) if e.rpc().is_some_and(|e| e.is("FRESH_CHANGE_PHONE_FORBIDDEN") || e.is("PHONE_NUMBER_OCCUPIED")) => {}
        Err(e) => panic!("{e}"),
    }
}
