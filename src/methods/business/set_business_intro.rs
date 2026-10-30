use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

impl Client {
    pub async fn set_business_intro(&self, title: impl Into<String>, message: impl Into<String>) -> Result<bool> {
        let intro = types::InputBusinessIntro { title: title.into(), description: message.into(), sticker: None };
        let intro = Some(base::InputBusinessIntro::InputBusinessIntro(Box::new(intro)));
        self.invoke(&functions::account::UpdateBusinessIntro { intro }).await
    }
}
