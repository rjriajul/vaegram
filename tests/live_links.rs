#[allow(dead_code)]
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn tg_resolve_link_reaches_the_chat() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let me = kit.bot.get_me().await.unwrap();
    let username = me.username.expect("the bot has no username");
    let by_name = kit.user.client.get_chat(username.as_str()).await.unwrap();
    for link in [format!("tg://resolve?domain={username}"), format!("https://telegram.dog/{username}")] {
        assert_eq!(kit.user.client.get_chat(link.as_str()).await.unwrap().id, by_name.id, "{link}");
    }
}
