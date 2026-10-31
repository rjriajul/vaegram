use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::User;

impl Client {
    pub async fn get_owned_bots(&self) -> Result<Vec<User>> {
        Ok(User::parse_all(self, self.invoke(&functions::bots::GetAdminedBots {}).await?))
    }
}

#[cfg(test)]
mod tests {
    use super::super::create_bot::tests::user;
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn lists_the_bots_and_drops_empty_entries() {
        let users = vec![user(77, true), base::User::UserEmpty(types::UserEmpty { id: 78 })];
        let (client, mut sent) = Client::scripted(vec![Ok(users.to_bytes())]);
        let bots = client.get_owned_bots().await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), functions::bots::GetAdminedBots {}.to_bytes());
        assert_eq!(bots.iter().map(|b| (b.id, b.is_bot)).collect::<Vec<_>>(), [(77, true)]);
    }

    #[tokio::test]
    async fn an_account_without_bots_gets_an_empty_list() {
        // Telegram answers an empty vector when the account owns no bots.
        let (client, _sent) = Client::scripted(vec![Ok(Vec::<base::User>::new().to_bytes())]);
        assert!(client.get_owned_bots().await.unwrap().is_empty());
    }
}
