use crate::Client;
use crate::errors::Result;
use crate::methods::{BlockUser, UnblockUser};
use crate::types::{Chat, Text, User};

/// Name helpers, and shortcuts that call the `Client` method of the same name with the user's id.
impl User {
    /// Returns the first and last name joined by a space, or the first name alone.
    pub fn full_name(&self) -> String {
        match &self.last_name {
            Some(last) => format!("{} {last}", self.first_name),
            None => self.first_name.clone(),
        }
    }

    /// Returns [`full_name`](User::full_name) as a text mention of this user.
    pub fn mention(&self) -> Text {
        Text::new().mention(self.full_name(), self.id)
    }

    /// Moves the chat with this user to the archive.
    pub async fn archive(&self, client: &Client) -> Result<()> {
        client.archive_chats([self.id]).await
    }

    /// Moves the chat with this user out of the archive.
    pub async fn unarchive(&self, client: &Client) -> Result<()> {
        client.unarchive_chats([self.id]).await
    }

    /// Blocks this user. Awaiting the returned [`BlockUser`] sends the request.
    pub fn block<'a>(&self, client: &'a Client) -> BlockUser<'a> {
        client.block_user(self.id)
    }

    /// Unblocks this user. Awaiting the returned [`UnblockUser`] sends the request.
    pub fn unblock<'a>(&self, client: &'a Client) -> UnblockUser<'a> {
        client.unblock_user(self.id)
    }

    /// Returns the chats you share with this user.
    pub async fn get_common_chats(&self, client: &Client) -> Result<Vec<Chat>> {
        client.get_common_chats(self.id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::enums::BlockList;
    use crate::raw::functions::{contacts, folders, messages};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::User;

    fn user() -> User {
        User { id: 7, first_name: "Ada".into(), last_name: Some("Lovelace".into()), ..User::default() }
    }

    #[test]
    fn full_name_and_mention_use_both_names() {
        let mut user = user();
        assert_eq!(user.full_name(), "Ada Lovelace");
        assert_eq!(user.mention().as_str(), "Ada Lovelace");
        assert_eq!(user.mention().entities().len(), 1);
        user.last_name = None;
        assert_eq!(user.full_name(), "Ada");
    }

    #[tokio::test]
    async fn user_helpers_send_their_requests() {
        let (client, mut rx) = Client::recording();
        let input = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 2 }));
        client.0.peers.lock().unwrap().insert(7, input.clone());
        let (user, rx) = (user(), &mut rx);

        assert_eq!(sent(rx, user.archive(&client)).await, folders::EditPeerFolders::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, user.unarchive(&client)).await, folders::EditPeerFolders::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, user.get_common_chats(&client)).await, messages::GetCommonChats::CONSTRUCTOR_ID);
        let _ = user.block(&client).block_list(BlockList::Stories).await;
        let block = contacts::Block { my_stories_from: true, id: input.clone() };
        assert_eq!(rx.try_recv().unwrap(), block.to_bytes());
        let _ = user.unblock(&client).await;
        assert_eq!(rx.try_recv().unwrap(), contacts::Unblock { my_stories_from: false, id: input }.to_bytes());
    }
}
