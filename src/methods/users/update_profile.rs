use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::User;

/// Request built by [`Client::update_profile`].
#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateProfile<'a> {
    client: &'a Client,
    first_name: Option<String>,
    last_name: Option<String>,
    bio: Option<String>,
}

impl Client {
    /// Changes your first name, last name or bio, and returns the updated [`User`].
    ///
    /// Only the fields you set change. Awaiting it with none set fails with `Error::InvalidArgument`.
    pub fn update_profile(&self) -> UpdateProfile<'_> {
        UpdateProfile { client: self, first_name: None, last_name: None, bio: None }
    }
}

impl UpdateProfile<'_> {
    /// Changes your first name.
    pub fn first_name(mut self, first_name: impl Into<String>) -> Self {
        self.first_name = Some(first_name.into());
        self
    }

    /// Changes your last name. An empty string removes it.
    pub fn last_name(mut self, last_name: impl Into<String>) -> Self {
        self.last_name = Some(last_name.into());
        self
    }

    /// Changes your bio. An empty string removes it.
    pub fn bio(mut self, bio: impl Into<String>) -> Self {
        self.bio = Some(bio.into());
        self
    }

    async fn send(self) -> Result<User> {
        if self.first_name.is_none() && self.last_name.is_none() && self.bio.is_none() {
            return Err(Error::InvalidArgument("set a first name, last name or bio to update"));
        }
        let request = functions::account::UpdateProfile {
            first_name: self.first_name,
            last_name: self.last_name,
            about: self.bio,
        };
        let user = self.client.invoke(&request).await?;
        Ok(User::from_raw(self.client, user))
    }
}

into_future!(UpdateProfile, User);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};

    #[tokio::test]
    async fn an_empty_update_is_refused_before_sending() {
        let (client, mut sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "FIRSTNAME_INVALID")))]);
        let result = client.update_profile().await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }
}
