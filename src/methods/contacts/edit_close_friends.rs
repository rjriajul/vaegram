use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Replaces your close friends list with these users.
    ///
    /// An empty list clears it. Fails with `Error::InvalidArgument` if an id names a chat or a channel.
    pub async fn edit_close_friends(&self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<()> {
        let mut id = Vec::new();
        for user in user_ids {
            let user_id = self.peer_id(user.into()).await?;
            if user_id <= 0 {
                return Err(Error::InvalidArgument("edit_close_friends takes users only"));
            }
            id.push(user_id);
        }
        accepted(self.invoke(&functions::contacts::EditCloseFriends { id }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::Serializable;
    use crate::raw::functions::contacts;

    #[tokio::test]
    async fn sends_the_user_ids_and_an_empty_list_clears() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.edit_close_friends([9, 10]).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), contacts::EditCloseFriends { id: vec![9, 10] }.to_bytes());
        client.edit_close_friends(Vec::<i64>::new()).await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), contacts::EditCloseFriends { id: Vec::new() }.to_bytes());
    }

    #[tokio::test]
    async fn a_chat_id_is_refused_without_a_request() {
        let (client, mut sent) = Client::recording();
        let result = client.edit_close_friends([9, -100_123]).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }
}
