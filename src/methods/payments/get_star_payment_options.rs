use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::StarPaymentOption;

impl Client {
    /// Lists the Star packages that can be bought.
    pub async fn get_star_payment_options(&self) -> Result<Vec<StarPaymentOption>> {
        let options = self.invoke(&functions::payments::GetStarsTopupOptions {}).await?;
        Ok(options.into_iter().map(StarPaymentOption::parse).collect())
    }
}
