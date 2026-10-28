use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};

impl Client {
    pub async fn transfer_business_account_stars(&self, business_connection_id: &str, star_count: i64) -> Result<()> {
        // Inside the connection inputUserSelf means the business user, so name the bot explicitly.
        let bot = match self.get_me().await? {
            base::User::User(u) => match u.access_hash {
                Some(access_hash) => types::InputUser { user_id: u.id, access_hash },
                None => return Err(Error::Unexpected("get_me returned the bot without an access hash")),
            },
            _ => return Err(Error::Unexpected("get_me returned an empty user")),
        };
        let invoice = base::InputInvoice::InputInvoiceBusinessBotTransferStars(Box::new(
            types::InputInvoiceBusinessBotTransferStars {
                bot: base::InputUser::InputUser(Box::new(bot)),
                stars: star_count,
            },
        ));
        let form = functions::payments::GetPaymentForm { invoice: invoice.clone(), theme_params: None };
        let form_id = match self.invoke_business(Some(business_connection_id), form).await?.0 {
            base::payments::PaymentForm::PaymentFormStars(f) => f.form_id,
            base::payments::PaymentForm::PaymentForm(f) => f.form_id,
            base::payments::PaymentForm::PaymentFormStarGift(f) => f.form_id,
        };
        let send = functions::payments::SendStarsForm { form_id, invoice };
        self.invoke_business(Some(business_connection_id), send).await?;
        Ok(())
    }
}
