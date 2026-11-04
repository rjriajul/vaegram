use vaegram::Client;
use vaegram::connection::Proxy;

/// Connects a logged-in user session through the proxy named by `VAEGRAM_PROXY_URL`; skipped when any of
/// `VAEGRAM_PROXY_URL`, `VAEGRAM_USER_SESSION`, `VAEGRAM_API_ID` or `VAEGRAM_API_HASH` is unset.
#[tokio::test]
#[ignore = "uses a proxy and a logged-in user session"]
async fn connects_through_parsed_proxy() {
    let vars = ["VAEGRAM_PROXY_URL", "VAEGRAM_USER_SESSION", "VAEGRAM_API_ID", "VAEGRAM_API_HASH"].map(std::env::var);
    let [Ok(url), Ok(session), Ok(api_id), Ok(api_hash)] = vars else {
        eprintln!("skipped: set VAEGRAM_PROXY_URL, VAEGRAM_USER_SESSION, VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return;
    };
    let proxy: Proxy = url.parse().expect("VAEGRAM_PROXY_URL isn't a proxy address");
    let client = Client::builder(session, api_id.parse().unwrap(), api_hash).proxy(proxy).connect().await.unwrap();
    client.get_me().await.unwrap();
}
