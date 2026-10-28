use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};

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
        self.pay_with_stars(Some(business_connection_id.into()), invoice).await
    }
}
