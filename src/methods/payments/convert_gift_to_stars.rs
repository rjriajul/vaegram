use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

/// Request built by [`Client::convert_gift_to_stars`].
#[must_use = "a request does nothing until it is awaited"]
pub struct ConvertGiftToStars<'a> {
    client: &'a Client,
    owned_gift_id: String,
    connection_id: Option<String>,
}

impl Client {
    /// Converts a received gift into Stars and gives up the gift.
    ///
    /// `owned_gift_id` is the `owned_gift_id` of an [`OwnedGift`](crate::types::OwnedGift) or a unique gift slug;
    /// awaiting fails with `Error::InvalidArgument` if it is empty.
    pub fn convert_gift_to_stars(&self, owned_gift_id: impl Into<String>) -> ConvertGiftToStars<'_> {
        ConvertGiftToStars { client: self, owned_gift_id: owned_gift_id.into(), connection_id: None }
    }
}

impl ConvertGiftToStars<'_> {
    /// Converts a gift of the business account behind this connection id.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        if self.owned_gift_id.is_empty() {
            return Err(Error::InvalidArgument("owned_gift_id is empty"));
        }
        let stargift = self.client.input_saved_gift(&self.owned_gift_id).await?;
        let request = functions::payments::ConvertStarGift { stargift };
        self.client.invoke_business(self.connection_id.as_deref(), request).await?;
        Ok(())
    }
}

into_future!(ConvertGiftToStars, ());
