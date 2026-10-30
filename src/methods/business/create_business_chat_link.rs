use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions, types};
use crate::types::{BusinessChatLink, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct CreateBusinessChatLink<'a> {
    client: &'a Client,
    text: Text,
    parse_mode: Option<ParseMode>,
    title: Option<String>,
}

impl Client {
    pub fn create_business_chat_link(&self, text: impl Into<Text>) -> CreateBusinessChatLink<'_> {
        CreateBusinessChatLink { client: self, text: text.into(), parse_mode: None, title: None }
    }
}

pub(super) async fn input_link(
    client: &Client,
    text: Text,
    parse_mode: Option<ParseMode>,
    title: Option<String>,
) -> Result<base::InputBusinessChatLink> {
    let (message, entities) = client.write_text(text, parse_mode).await?;
    let entities = (!entities.is_empty()).then_some(entities);
    let link = types::InputBusinessChatLink { message, entities, title };
    Ok(base::InputBusinessChatLink::InputBusinessChatLink(Box::new(link)))
}

pub(super) fn check_slug(slug: &str) -> Result<()> {
    if slug.is_empty() {
        return Err(Error::InvalidArgument("a business chat link needs its slug"));
    }
    Ok(())
}

impl CreateBusinessChatLink<'_> {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    async fn send(self) -> Result<BusinessChatLink> {
        let link = input_link(self.client, self.text, self.parse_mode, self.title).await?;
        let link = self.client.invoke(&functions::account::CreateBusinessChatLink { link }).await?;
        Ok(BusinessChatLink::parse(link, &Entities::default()))
    }
}

caption_setters!(CreateBusinessChatLink);
into_future!(CreateBusinessChatLink, BusinessChatLink);
