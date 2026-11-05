use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::NoPaidMessagesToken;

/// Makes a link that lets whoever opens it message you without paying.
/// Request built by [`Client::export_no_paid_messages_token`].
#[must_use = "a request does nothing until it is awaited"]
pub struct ExportNoPaidMessagesToken<'a> {
    client: &'a Client,
    revoke: bool,
    temporary: bool,
}

impl Client {
    /// Creates a link that lets whoever opens it message you without paying. Awaiting it returns the token.
    pub fn export_no_paid_messages_token(&self) -> ExportNoPaidMessagesToken<'_> {
        ExportNoPaidMessagesToken { client: self, revoke: false, temporary: false }
    }
}

impl ExportNoPaidMessagesToken<'_> {
    /// Cancels the links made so far instead of keeping them.
    pub fn revoke(mut self) -> Self {
        self.revoke = true;
        self
    }

    /// Makes a link that expires.
    pub fn temporary(mut self) -> Self {
        self.temporary = true;
        self
    }

    async fn send(self) -> Result<NoPaidMessagesToken> {
        let request = functions::contacts::ExportNoPaidMessagesToken { revoke: self.revoke, temporary: self.temporary };
        Ok(NoPaidMessagesToken::parse(self.client.invoke(&request).await?))
    }
}

into_future!(ExportNoPaidMessagesToken, NoPaidMessagesToken);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn export_maps_url_to_link() {
        let raw = types::ExportedNoPaidMessagesToken {
            token: "abc".into(),
            url: "https://t.me/me?free=abc".into(),
            expires: Some(99),
        };
        let reply = base::ExportedNoPaidMessagesToken::ExportedNoPaidMessagesToken(Box::new(raw)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);
        let token = client.export_no_paid_messages_token().temporary().await.unwrap();
        assert_eq!(
            (token.token.as_str(), token.link.as_str(), token.expiration_date),
            ("abc", "https://t.me/me?free=abc", Some(99))
        );
        let request = functions::contacts::ExportNoPaidMessagesToken { revoke: false, temporary: true };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
