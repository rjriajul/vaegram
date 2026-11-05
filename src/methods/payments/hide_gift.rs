use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Hides a received gift from the profile.
    ///
    /// `owned_gift_id` is the `owned_gift_id` of an owned gift, or a unique gift's slug.
    /// [`Client::show_gift`] undoes it.
    pub async fn hide_gift(&self, owned_gift_id: &str) -> Result<()> {
        let stargift = self.input_saved_gift(owned_gift_id).await?;
        self.invoke(&functions::payments::SaveStarGift { unsave: true, stargift }).await?;
        Ok(())
    }
}
