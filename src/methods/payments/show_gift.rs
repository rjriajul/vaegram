use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Shows a received gift on the profile again.
    ///
    /// `owned_gift_id` is the saved gift id or the unique gift slug. [`Client::hide_gift`] undoes it.
    pub async fn show_gift(&self, owned_gift_id: &str) -> Result<()> {
        let stargift = self.input_saved_gift(owned_gift_id).await?;
        self.invoke(&functions::payments::SaveStarGift { unsave: false, stargift }).await?;
        Ok(())
    }
}
