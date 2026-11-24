// Run with `cargo test --test live_w4_regex --features regex -- --ignored`; see tests/support/mod.rs for the env.
#![cfg(feature = "regex")]

#[allow(dead_code)]
mod support;

use regex::Regex;
use vaegram::filters;

#[tokio::test]
#[ignore = "needs a logged-in user session and a bot"]
async fn regex_captures_an_order_number() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let order = Regex::new(r"order (\d+)").unwrap();
    let pattern = order.clone();
    kit.bot.on_message(filters::private() & filters::regex(order), move |m| {
        let number = filters::matches(&pattern, &m).first().map(|found| found[1].to_owned());
        async move {
            m.reply(number.unwrap_or_default()).await?;
            Ok(())
        }
    });
    let reply = kit.user.send_and_wait_reply(&kit.bot, "order 42").await.unwrap();
    assert_eq!(reply.text(), "42");
}
