use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ShareThemeBackdrop;

impl Client {
    /// The backdrops [`set_share_theme`](Client::set_share_theme) accepts.
    pub async fn get_share_theme_backdrops(&self) -> Result<Vec<ShareThemeBackdrop>> {
        match self.invoke(&functions::messages::GetShareThemeBackdrops { hash: 0 }).await? {
            base::ShareThemeBackdrops::ShareThemeBackdrops(b) => {
                Ok(b.backdrops.iter().map(ShareThemeBackdrop::parse).collect())
            }
            // Only answered to a hash we never send.
            base::ShareThemeBackdrops::ShareThemeBackdropsNotModified => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn backdrops_parse() {
        let option = types::ShareThemeBackdropOption {
            backdrop_id: 3,
            name: "Dusk".into(),
            center_color: 1,
            edge_color: 2,
            pattern_color: 3,
            text_color: 4,
        };
        let reply = types::ShareThemeBackdrops {
            hash: 9,
            backdrops: vec![base::ShareThemeBackdropOption::ShareThemeBackdropOption(Box::new(option))],
        };
        let reply = base::ShareThemeBackdrops::ShareThemeBackdrops(Box::new(reply)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);
        let backdrops = client.get_share_theme_backdrops().await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), functions::messages::GetShareThemeBackdrops { hash: 0 }.to_bytes());
        let b = &backdrops[0];
        assert_eq!((b.id, b.name.as_str(), b.center_color, b.edge_color), (3, "Dusk", 1, 2));
        assert_eq!((b.pattern_color, b.text_color), (3, 4));
    }
}
