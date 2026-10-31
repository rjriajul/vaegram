#[allow(dead_code)]
mod support;

use vaegram::enums::ParseMode;
use vaegram::raw::{base, functions, types};
use vaegram::types::InputMedia;
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

#[tokio::test]
#[ignore = "uses a logged-in user session; creates and deletes a channel"]
async fn user_checks_usernames_and_lists_owned_bots() {
    let Some(user) = user_client().await else { return };
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("no account") };
    if let Some(own) = me.username {
        // Telegram's answer for the account's own name isn't documented; only a crash would be wrong.
        let result = user.check_username("me", own).await;
        assert!(result.is_ok() || result.as_ref().is_err_and(|e| e.rpc().is_some()), "{result:?}");
    }
    let channel = user.create_channel("vaegram username check", "").await.unwrap();
    let free = user.check_username(channel.id, format!("vaegram_free_{}", test_number())).await;
    user.delete_channel(channel.id).await.unwrap();
    assert!(free.unwrap());

    let taken = user.check_bot_username("BotFather").await;
    assert!(matches!(taken, Ok(false)) || taken.as_ref().is_err_and(|e| e.rpc().is_some()), "{taken:?}");
    for bot in user.get_owned_bots().await.unwrap() {
        assert!(bot.is_bot, "{bot:?}");
    }
}

#[tokio::test]
#[ignore = "creates a managed bot that stays on the account"]
async fn user_creates_a_managed_bot() {
    if std::env::var("VAEGRAM_CREATE_BOT").as_deref() != Ok("1") {
        eprintln!("skipped: set VAEGRAM_CREATE_BOT=1 to create a bot that stays on the account");
        return;
    }
    let Some(kit) = support::Kit::from_env().await else { return };
    let base::User::User(manager) = kit.bot.get_me().await.unwrap() else { panic!("the bot has no user") };
    let name = manager.username.expect("the kit's bot has no username");
    let manager = kit.user.client.get_users([format!("@{name}")]).await.unwrap().remove(0);
    if !manager.can_manage_bots {
        eprintln!("skipped: the kit's bot can't manage bots");
        return;
    }
    let username = format!("vaegram_{}_bot", test_number());
    let bot = kit.user.client.create_bot(manager.id, "vaegram managed", username.as_str()).await.unwrap();
    assert!(bot.is_bot);
    assert_eq!(bot.username.as_deref(), Some(username.as_str()));
}

fn env_id(key: &str) -> Option<i64> {
    let id = std::env::var(key).ok().and_then(|v| v.parse().ok());
    if id.is_none() {
        eprintln!("skipped: set {key} (a user id)");
    }
    id
}

async fn contact_note(user: &Client, user_id: i64) -> Option<String> {
    let base::InputPeer::InputPeerUser(peer) = user.resolve_peer(user_id).await.unwrap() else { panic!("not a user") };
    let id = base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: peer.access_hash }));
    let base::users::UserFull::UserFull(full) = user.invoke(&functions::users::GetFullUser { id }).await.unwrap();
    let base::UserFull::UserFull(full) = full.full_user;
    full.note.map(|base::TextWithEntities::TextWithEntities(note)| note.text)
}

#[tokio::test]
#[ignore = "uses a logged-in user session; changes a contact's note and puts back its text without formatting"]
async fn user_sets_a_contact_note() {
    let Some(contact) = env_id("VAEGRAM_CONTACT_ID") else { return };
    let Some(user) = user_client().await else { return };
    let old = contact_note(&user, contact).await.unwrap_or_default();

    user.set_contact_note(contact, "<b>vaegram</b> note").html().await.unwrap();
    let set = contact_note(&user, contact).await;
    user.set_contact_note(contact, old.as_str()).parse_mode(ParseMode::Disabled).await.unwrap();
    assert_eq!(set.as_deref(), Some("vaegram note"));
}

#[tokio::test]
#[ignore = "uses a logged-in user session; sends a birthday suggestion"]
async fn user_suggests_a_birthday() {
    let Some(target) = env_id("VAEGRAM_BIRTHDAY_USER_ID") else { return };
    let Some(user) = user_client().await else { return };
    match user.suggest_birthday(target, 1, 1, None).await {
        Ok(()) => {}
        // The user already shows a birthday, so Telegram doesn't take a suggestion.
        Err(e) if rpc_is(&e, "BIRTHDAY_ALREADY") => {}
        Err(e) => panic!("{e}"),
    }
}

#[tokio::test]
#[ignore = "uses a logged-in user session; adds two songs to the profile and removes them"]
async fn user_adds_moves_and_removes_profile_music() {
    let Ok(path) = std::env::var("VAEGRAM_AUDIO") else {
        eprintln!("skipped: set VAEGRAM_AUDIO (an audio file path)");
        return;
    };
    let Some(user) = user_client().await else { return };
    let before = user.get_user_profile_audios("me").await.unwrap();
    let known: Vec<i64> = before.audios.iter().map(|a| a.id).collect();

    for title in ["vaegram one", "vaegram two"] {
        user.add_profile_audio(&InputMedia::audio(path.as_str()).title(title)).await.unwrap();
    }
    let added = user.get_user_profile_audios("me").await.unwrap();
    let new: Vec<_> = added.audios.into_iter().filter(|a| !known.contains(&a.id)).collect();
    assert_eq!(new.len(), 2, "{new:?}");

    user.set_profile_audio_position(&new[0], Some(&new[1])).await.unwrap();
    let moved = user.get_user_profile_audios("me").await.unwrap();
    let order: Vec<i64> = moved.audios.iter().map(|a| a.id).filter(|id| new.iter().any(|n| n.id == *id)).collect();
    user.set_profile_audio_position(&new[0], None).await.unwrap();
    for song in &new {
        user.remove_profile_audio(song).await.unwrap();
    }
    let after = user.get_user_profile_audios("me").await.unwrap();
    assert_eq!(order, [new[1].id, new[0].id]);
    assert_eq!(after.total_count, before.total_count);
}
