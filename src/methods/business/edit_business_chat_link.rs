use super::create_business_chat_link::{check_slug, input_link};
use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::functions;
use crate::types::{BusinessChatLink, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditBusinessChatLink<'a> {
    client: &'a Client,
    slug: String,
    text: Text,
    parse_mode: Option<ParseMode>,
    title: Option<String>,
}

impl Client {
    pub fn edit_business_chat_link(&self, slug: impl Into<String>, text: impl Into<Text>) -> EditBusinessChatLink<'_> {
        EditBusinessChatLink { client: self, slug: slug.into(), text: text.into(), parse_mode: None, title: None }
    }
}

impl EditBusinessChatLink<'_> {
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    async fn send(self) -> Result<BusinessChatLink> {
        check_slug(&self.slug)?;
        let link = input_link(self.client, self.text, self.parse_mode, self.title).await?;
        let link = self.client.invoke(&functions::account::EditBusinessChatLink { slug: self.slug, link }).await?;
        Ok(BusinessChatLink::parse(link, &Entities::default()))
    }
}

caption_setters!(EditBusinessChatLink);
into_future!(EditBusinessChatLink, BusinessChatLink);
