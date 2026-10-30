use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::User;

impl Client {
    pub async fn sign_up(
        &self,
        phone_number: &str,
        phone_code_hash: &str,
        first_name: &str,
        last_name: &str,
    ) -> Result<User> {
        let request = functions::auth::SignUp {
            no_joined_notifications: false,
            phone_number: phone_number.into(),
            phone_code_hash: phone_code_hash.into(),
            first_name: first_name.into(),
            last_name: last_name.into(),
        };
        let auth = self.invoke(&request).await?;
        self.authorized_user(auth)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn sends_the_names_with_the_code_hash() {
        let (client, mut sent) = Client::recording();
        let _ = client.sign_up("8801", "hash", "Ada", "").await;
        let expected = functions::auth::SignUp {
            no_joined_notifications: false,
            phone_number: "8801".into(),
            phone_code_hash: "hash".into(),
            first_name: "Ada".into(),
            last_name: String::new(),
        };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }
}
