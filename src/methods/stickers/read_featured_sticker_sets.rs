use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    pub async fn read_featured_sticker_sets(&self, set_ids: &[i64]) -> Result<()> {
        if set_ids.is_empty() {
            return Err(Error::InvalidArgument("read_featured_sticker_sets takes at least one set id"));
        }
        self.invoke(&functions::messages::ReadFeaturedStickers { id: set_ids.to_vec() }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn no_ids_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.read_featured_sticker_sets(&[]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
