use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

#[must_use = "a request does nothing until it is awaited"]
pub struct ConvertGiftToStars<'a> {
    client: &'a Client,
    owned_gift_id: String,
    connection_id: Option<String>,
}

impl Client {
    pub fn convert_gift_to_stars(&self, owned_gift_id: impl Into<String>) -> ConvertGiftToStars<'_> {
        ConvertGiftToStars { client: self, owned_gift_id: owned_gift_id.into(), connection_id: None }
    }
}

impl ConvertGiftToStars<'_> {
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<bool> {
        let stargift = self.client.input_saved_gift(&self.owned_gift_id).await?;
        let request = functions::payments::ConvertStarGift { stargift };
        Ok(self.client.invoke_business(self.connection_id.as_deref(), request).await?.0)
    }
}

into_future!(ConvertGiftToStars, bool);
