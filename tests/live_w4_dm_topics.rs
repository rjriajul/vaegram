// Run with `cargo test --test live_w4_dm_topics -- --ignored`; see tests/support/mod.rs for the env.
// Live behaviour is unverified: this test has not been run.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore]
async fn lists_topics_and_reads_history() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Some(chat) = std::env::var("VAEGRAM_DM_CHAT_ID").ok().and_then(|v| v.parse::<i64>().ok()) else {
        eprintln!("skipped: set VAEGRAM_DM_CHAT_ID (a channel's direct messages chat)");
        return;
    };
    let user = kit.user.client;
    let mut topics = user.get_direct_messages_topics(chat).limit(5);
    let Some(topic) = topics.next().await.unwrap() else {
        eprintln!("skipped: the direct messages chat has no topics");
        return;
    };
    let id = topic.topic.topic_id;
    let mut history = user.get_direct_messages_chat_topic_history(chat, id).limit(5);
    while history.next().await.unwrap().is_some() {}
}

/// Destructive: deletes the whole history of the first topic of VAEGRAM_DM_CHAT_ID, so use a disposable one.
/// The delete's pts is not applied to the update tracker, so the history must read back empty afterwards.
#[tokio::test]
#[ignore]
async fn deleting_a_topic_history_leaves_it_empty() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Some(chat) = std::env::var("VAEGRAM_DM_CHAT_ID").ok().and_then(|v| v.parse::<i64>().ok()) else {
        eprintln!("skipped: set VAEGRAM_DM_CHAT_ID (a disposable direct messages chat)");
        return;
    };
    let user = kit.user.client;
    let Some(topic) = user.get_direct_messages_topics(chat).limit(1).next().await.unwrap() else {
        eprintln!("skipped: the direct messages chat has no topics");
        return;
    };
    let id = topic.topic.topic_id;
    user.delete_direct_messages_chat_topic_history(chat, id).await.unwrap();
    assert!(user.get_direct_messages_chat_topic_history(chat, id).limit(1).next().await.unwrap().is_none());
}
