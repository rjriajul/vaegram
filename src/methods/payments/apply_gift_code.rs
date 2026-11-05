use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

use super::check_gift_code::gift_code_slug;

impl Client {
    /// Redeems a gift code for Premium on the current account.
    ///
    /// `slug` is a bare slug or a `t.me/giftcode/...` or `tg://giftcode?slug=...` link. Fails with `InvalidArgument`
    /// if no slug can be read from it.
    pub async fn apply_gift_code(&self, slug: &str) -> Result<()> {
        self.invoke(&functions::payments::ApplyGiftCode { slug: gift_code_slug(slug)?.into() }).await.map(drop)
    }
}
