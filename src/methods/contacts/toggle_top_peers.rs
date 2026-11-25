use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Turns the list of frequently used chats, which Telegram suggests when you share or search, on or off.
    ///
    /// Turning it off makes Telegram forget the ratings it has collected.
    pub async fn toggle_top_peers(&self, enabled: bool) -> Result<()> {
        accepted(self.invoke(&functions::contacts::ToggleTopPeers { enabled }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::Serializable;
    use crate::raw::functions::contacts;

    #[tokio::test]
    async fn sends_the_flag_both_ways() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        for enabled in [false, true] {
            client.toggle_top_peers(enabled).await.unwrap();
            assert_eq!(sent.try_recv().unwrap(), contacts::ToggleTopPeers { enabled }.to_bytes());
        }
    }
}
