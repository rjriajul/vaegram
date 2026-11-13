// Run with `cargo test --test live_media_dc -- --ignored`; needs VAEGRAM_API_ID, VAEGRAM_API_HASH and
// VAEGRAM_BOT_TOKEN, and skips itself without them.
//
// Downloads go to a data center's media-only address when help.getConfig lists one (a `media_only` option). There
// Telegram wants the DC's id negated, in the auth-key handshake (`p_q_inner_data_dc`, and `p_q_inner_data_temp_dc`
// for a PFS key) and in the obfuscation header; a positive id is closed with transport error -444. Each test below
// carries the id one of these ways.

#[allow(dead_code)] // this binary only logs the bot in
mod support;

use vaegram::connection::Proxy;
use vaegram::raw::{base, functions};
use vaegram::{Client, ClientBuilder};

/// Public chats whose profile photos the tests download: the first one stored on a data center that fits is used.
const PUBLIC_CHATS: [&str; 12] = [
    "telegram",
    "durov",
    "BotNews",
    "TelegramTips",
    "designers",
    "contest",
    "BotFather",
    "stickers",
    "gif",
    "pic",
    "wiki",
    "PremiumBot",
];

/// The test bot on storage of its own, or `None` (after saying so) when the env isn't set.
fn bot(storage: &str) -> Option<ClientBuilder> {
    let vars = ["VAEGRAM_API_ID", "VAEGRAM_API_HASH", "VAEGRAM_BOT_TOKEN"].map(std::env::var);
    let [Ok(api_id), Ok(api_hash), Ok(token)] = vars else {
        eprintln!("skipped: set VAEGRAM_API_ID, VAEGRAM_API_HASH and VAEGRAM_BOT_TOKEN");
        return None;
    };
    Some(Client::builder(storage, api_id.parse().unwrap(), api_hash).bot_token(token))
}

/// The data center the client is connected to, and the ones with a media-only IPv4 address.
async fn data_centers(client: &Client) -> (i32, Vec<i32>) {
    let base::Config::Config(config) = client.invoke(&functions::help::GetConfig {}).await.unwrap();
    let media = config
        .dc_options
        .iter()
        .filter_map(|base::DcOption::DcOption(o)| (o.media_only && !o.ipv6 && !o.cdn && !o.tcpo_only).then_some(o.id))
        .collect();
    (config.this_dc, media)
}

/// The big profile photo of the first public chat whose photo is on a data center `wanted` accepts, and that DC.
async fn chat_photo(client: &Client, wanted: impl Fn(i32) -> bool) -> Option<(String, i32)> {
    for username in PUBLIC_CHATS {
        let Ok(chat) = client.get_chat(username).await else { continue };
        let (Some(photo), Some(dc_id)) = (chat.photo, chat.dc_id) else { continue };
        if wanted(dc_id) {
            return Some((photo.big_file_id(), dc_id));
        }
    }
    eprintln!("skipped: none of {PUBLIC_CHATS:?} has a profile photo on a data center that fits");
    None
}

/// Logs the bot in with `builder` and downloads a photo through a media-only address: on a DC other than the bot's
/// own when `foreign`, so the download starts with a handshake there.
async fn downloads_from_a_media_dc(builder: ClientBuilder, foreign: bool) {
    let Some(bot) = support::start_bot(builder).await else { return };
    let _disconnect = support::Disconnect(vec![bot.clone()]);
    let (home, media) = data_centers(&bot).await;
    let wanted = |dc_id| media.contains(&dc_id) && (!foreign || dc_id != home);
    let Some((file_id, dc_id)) = chat_photo(&bot, wanted).await else { return };
    let bytes = bot.download_media(&file_id).await;
    let bytes = bytes.unwrap_or_else(|e| panic!("downloading from DC {dc_id}'s media-only address failed: {e:?}"));
    assert!(!bytes.is_empty());
}

#[tokio::test]
#[ignore = "logs in as a bot and downloads from a media-only address"]
async fn a_handshake_on_a_media_dc_sends_the_negative_id() {
    let Some(builder) = bot("target/vaegram-media-dc-bot") else { return };
    downloads_from_a_media_dc(builder, true).await;
}

#[tokio::test]
#[ignore = "logs in as a bot and downloads from a media-only address"]
async fn a_temporary_key_on_a_media_dc_sends_the_negative_id() {
    let Some(builder) = bot("target/vaegram-media-dc-pfs-bot") else { return };
    downloads_from_a_media_dc(builder.pfs(), false).await;
}

#[tokio::test]
#[ignore = "logs in as a bot and downloads from a media-only address"]
async fn the_obfuscation_header_to_a_media_dc_sends_the_negative_id() {
    let Some(builder) = bot("target/vaegram-media-dc-obfuscated-bot") else { return };
    downloads_from_a_media_dc(builder.obfuscated(), false).await;
}

/// An MTProxy picks the data center from the header alone, so this checks the proxy takes the negative id.
#[tokio::test]
#[ignore = "uses an MTProxy, logs in as a bot and downloads from a media-only address"]
async fn an_mtproxy_takes_the_negative_id() {
    let Ok(url) = std::env::var("VAEGRAM_PROXY_URL") else {
        eprintln!("skipped: set VAEGRAM_PROXY_URL to an MTProxy");
        return;
    };
    let proxy: Proxy = url.parse().expect("VAEGRAM_PROXY_URL isn't a proxy address");
    if !matches!(proxy, Proxy::MtProxy { .. }) {
        eprintln!("skipped: VAEGRAM_PROXY_URL isn't an MTProxy");
        return;
    }
    let Some(builder) = bot("target/vaegram-media-dc-mtproxy-bot") else { return };
    downloads_from_a_media_dc(builder.proxy(proxy), false).await;
}
