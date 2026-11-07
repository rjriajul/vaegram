#[allow(dead_code)]
mod support;

use vaegram::Client;
use vaegram::enums::{StickerType, StoriesPrivacyRules};
use vaegram::raw::{base, functions, types};
use vaegram::types::InputStoryContent;

async fn user_client() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    Some(client.connect().await.unwrap())
}

#[tokio::test]
#[ignore = "posts, forwards, copies and deletes stories with a logged-in user session"]
async fn user_checks_forwards_and_copies_a_story() {
    let Ok(photo) = std::env::var("VAEGRAM_STORY_PHOTO") else {
        eprintln!("skipped: set VAEGRAM_STORY_PHOTO (a JPEG path) and VAEGRAM_USER_SESSION");
        return;
    };
    let Some(user) = user_client().await else { return };
    let me = user.get_me().await.unwrap();
    assert!(user.get_story_post_count(me.id).await.unwrap() > 0);

    // Telegram forwards only public stories (STORY_PUBLIC_MISSING otherwise), and everyone sees a public story while
    // it's up, so the forward runs only with VAEGRAM_POST_STORIES=1.
    let public = std::env::var("VAEGRAM_POST_STORIES").is_ok_and(|v| v == "1");
    let only_me = StoriesPrivacyRules::SelectedUsers(vec![me.id]);
    let privacy = if public { StoriesPrivacyRules::default() } else { only_me.clone() };
    let story = match user.send_story(me.id, InputStoryContent::photo(photo.as_str())).privacy(privacy).await {
        Ok(story) => story,
        Err(e) if support::story_limit_reached(&e) => return,
        Err(e) => panic!("{e:?}"),
    };

    if public {
        let forwarded = user.forward_story("me", me.id, story.id).await.unwrap();
        assert_eq!(forwarded.chat.id, me.id);
        user.delete_messages("me", &[forwarded.id]).await.unwrap();
    } else {
        eprintln!("skipped the forward: Telegram forwards only public stories; set VAEGRAM_POST_STORIES=1 to post one");
    }

    let copy = match user.copy_story(me.id, me.id, story.id).caption("copy").privacy(only_me).await {
        Ok(copy) => copy,
        Err(e) if support::story_limit_reached(&e) => {
            user.delete_stories(me.id, &[story.id]).await.unwrap();
            return;
        }
        Err(e) => panic!("{e:?}"),
    };
    assert_ne!(copy.id, story.id);
    assert_eq!(copy.caption.as_deref(), Some("copy"));
    assert!(copy.media.is_some());

    user.delete_stories(me.id, &[story.id, copy.id]).await.unwrap();
}

#[tokio::test]
#[ignore = "reorders and restores the user's installed sticker sets with a logged-in user session"]
async fn user_reorders_sticker_sets_and_restores_them() {
    let Some(user) = user_client().await else { return };
    assert!(!user.get_suggested_sticker_set_name("vaegram test").await.unwrap().is_empty());
    user.get_owned_sticker_sets().limit(10).await.unwrap();

    let ids = |sets: Vec<vaegram::types::StickerSet>| sets.into_iter().map(|s| s.id).collect::<Vec<_>>();
    let original = ids(user.get_installed_sticker_sets(StickerType::Regular).await.unwrap());
    if original.len() < 2 {
        eprintln!("skipped the reorder: it needs two installed sticker sets");
        return;
    }
    let reversed: Vec<i64> = original.iter().rev().copied().collect();
    user.reorder_installed_sticker_sets(StickerType::Regular, &reversed).await.unwrap();
    assert_eq!(ids(user.get_installed_sticker_sets(StickerType::Regular).await.unwrap()), reversed);
    user.reorder_installed_sticker_sets(StickerType::Regular, &original).await.unwrap();
    assert_eq!(ids(user.get_installed_sticker_sets(StickerType::Regular).await.unwrap()), original);
}

#[tokio::test]
#[ignore = "needs the kit bot and user in VAEGRAM_CHAT_ID; deletes the bot's welcome messages there"]
async fn bot_stores_and_deletes_a_welcome_message() {
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID (a group with the kit bot and user)");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let Some(kit) = support::Kit::from_env().await else { return };
    let (user, bot) = (&kit.user.client, &kit.bot);
    // A bot sends welcome messages only with the admin right for them (Bot API `can_send_welcome_messages`).
    let bot_me = bot.get_me().await.unwrap();
    let rights = bot.get_chat_member(chat_id, bot_me.id).await.unwrap().privileges;
    if !rights.is_some_and(|r| r.can_send_welcome_messages) {
        eprintln!("skipped: the bot isn't an admin of VAEGRAM_CHAT_ID with the right to send welcome messages");
        return;
    }
    let me = user.get_me().await.unwrap();

    let base::InputPeer::InputPeerUser(receiver) = bot.resolve_peer(me.id).await.unwrap() else {
        panic!("the bot can't address the kit user")
    };
    let receiver = types::InputUser { user_id: receiver.user_id, access_hash: receiver.access_hash };
    let random_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as i64;
    let request = functions::ephemeral::SendMessage {
        invert_media: false,
        welcome: true,
        anchor: false,
        noforwards: false,
        peer: Some(bot.resolve_peer(chat_id).await.unwrap()),
        receiver_id: base::InputUser::InputUser(Box::new(receiver)),
        query_id: None,
        message: "welcome".into(),
        entities: None,
        media: None,
        reply_markup: None,
        rich_message: None,
        random_id,
        reply_to: None,
    };
    bot.invoke(&request).await.unwrap();

    let stored = user.get_welcome_messages(chat_id).await.unwrap();
    let welcome = stored.iter().find(|m| m.text == "welcome").expect("the welcome message wasn't stored");
    bot.delete_welcome_message(chat_id, welcome.id).await.unwrap();
    assert!(!user.get_welcome_messages(chat_id).await.unwrap().iter().any(|m| m.id == welcome.id));
    bot.delete_all_welcome_messages(chat_id).await.unwrap();
}

#[tokio::test]
#[ignore = "needs VAEGRAM_CALL_CHAT_ID: a group with an active video chat"]
async fn user_lists_group_call_members() {
    let Ok(chat_id) = std::env::var("VAEGRAM_CALL_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CALL_CHAT_ID (a group with an active video chat) and VAEGRAM_USER_SESSION");
        return;
    };
    let chat_id: i64 = chat_id.parse().unwrap();
    let Some(user) = user_client().await else { return };
    let mut members = user.get_call_members(chat_id).limit(5);
    let mut count = 0;
    while let Some(member) = members.next().await.unwrap() {
        assert!(member.date > 0);
        count += 1;
    }
    assert!(count <= 5);
}

#[tokio::test]
#[ignore = "needs a user account with Telegram's TON wallet available"]
async fn user_looks_up_ton_wallet_addresses() {
    let Some(user) = user_client().await else { return };
    let found = match user.get_user_ton_wallet_addresses(["me"]).await {
        Ok(found) => found,
        Err(e) if e.is("WALLET_UNAVAILABLE") => {
            eprintln!("skipped: Telegram's wallet isn't available to this account (WALLET_UNAVAILABLE)");
            return;
        }
        Err(e) => panic!("{e:?}"),
    };
    let Some(first) = found.first() else {
        eprintln!("skipped the address lookup: the account has no wallet");
        return;
    };
    let by_address =
        user.get_address_ton_wallet(&first.wallet_address).await.unwrap().expect("no wallet at the address");
    assert_eq!(by_address.public_key, first.public_key);
}
