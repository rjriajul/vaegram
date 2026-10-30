use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::User;

impl Client {
    pub async fn recover_password(&self, recovery_code: &str) -> Result<User> {
        let request = functions::auth::RecoverPassword { code: recovery_code.into(), new_settings: None };
        let auth = self.invoke(&request).await?;
        self.authorized_user(auth)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn sends_only_the_code() {
        let (client, mut sent) = Client::recording();
        let _ = client.recover_password("12345").await;
        let expected = functions::auth::RecoverPassword { code: "12345".into(), new_settings: None };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }
}
