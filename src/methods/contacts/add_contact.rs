use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

/// Request built by [`Client::add_contact`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AddContact<'a> {
    client: &'a Client,
    user_id: PeerRef,
    first_name: String,
    last_name: String,
    phone_number: String,
    share_phone_number: bool,
}

impl Client {
    /// Adds a user to your contacts under the given first name.
    pub fn add_contact(&self, user_id: impl Into<PeerRef>, first_name: impl Into<String>) -> AddContact<'_> {
        AddContact {
            client: self,
            user_id: user_id.into(),
            first_name: first_name.into(),
            last_name: String::new(),
            phone_number: String::new(),
            share_phone_number: false,
        }
    }
}

impl AddContact<'_> {
    /// Stores a last name for the contact. Empty by default.
    pub fn last_name(mut self, last_name: impl Into<String>) -> Self {
        self.last_name = last_name.into();
        self
    }

    /// Stores a phone number for the contact. Empty by default.
    pub fn phone_number(mut self, phone_number: impl Into<String>) -> Self {
        self.phone_number = phone_number.into();
        self
    }

    /// Adds the user as an exception to your phone number privacy, so they can see your number.
    pub fn share_phone_number(mut self) -> Self {
        self.share_phone_number = true;
        self
    }

    async fn send(self) -> Result<()> {
        let request = functions::contacts::AddContact {
            add_phone_privacy_exception: self.share_phone_number,
            id: self.client.resolve_user(&self.user_id).await?,
            first_name: self.first_name,
            last_name: self.last_name,
            phone: self.phone_number,
            note: None,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(AddContact, ());
