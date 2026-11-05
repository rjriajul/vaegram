use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Sticker;

/// Request built by [`Client::get_recent_stickers`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetRecentStickers<'a> {
    client: &'a Client,
    attached: bool,
}

impl Client {
    /// Lists the stickers the account used recently.
    pub fn get_recent_stickers(&self) -> GetRecentStickers<'_> {
        GetRecentStickers { client: self, attached: false }
    }
}

impl GetRecentStickers<'_> {
    /// The stickers recently attached to photos and videos instead of the ones recently sent.
    pub fn attached(mut self) -> Self {
        self.attached = true;
        self
    }

    async fn send(self) -> Result<Vec<Sticker>> {
        let request = functions::messages::GetRecentStickers { attached: self.attached, hash: 0 };
        Ok(match self.client.invoke(&request).await? {
            base::messages::RecentStickers::RecentStickers(r) => r.stickers.iter().filter_map(Sticker::parse).collect(),
            base::messages::RecentStickers::RecentStickersNotModified => Vec::new(),
        })
    }
}

into_future!(GetRecentStickers, Vec<Sticker>);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::messages::GetRecentStickers;
    use crate::raw::{Serializable, base};

    #[tokio::test]
    async fn attached_sets_the_flag() {
        let none = || Ok(base::messages::RecentStickers::RecentStickersNotModified.to_bytes());
        let (client, mut rx) = Client::scripted(vec![none(), none()]);
        assert!(client.get_recent_stickers().await.unwrap().is_empty());
        assert!(client.get_recent_stickers().attached().await.unwrap().is_empty());
        let sent = [rx.try_recv().unwrap(), rx.try_recv().unwrap()];
        let request = |attached| GetRecentStickers { attached, hash: 0 }.to_bytes();
        assert_eq!(sent, [request(false), request(true)]);
    }
}
