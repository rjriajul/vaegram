use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

use super::check_gift_code::gift_code_slug;

impl Client {
    pub async fn apply_gift_code(&self, slug: &str) -> Result<()> {
        self.invoke(&functions::payments::ApplyGiftCode { slug: gift_code_slug(slug)?.into() }).await.map(drop)
    }
}
