use crate::Client;
use crate::errors::Result;
use crate::methods::CreateFolderInviteLink;
use crate::types::{Folder, PeerRef};

fn add(list: &mut Vec<i64>, id: i64) {
    if !list.contains(&id) {
        list.push(id);
    }
}

fn remove(list: &mut Vec<i64>, id: i64) {
    list.retain(|c| *c != id);
}

/// Shortcuts for the `Client` folder methods.
///
/// The chat helpers change a copy of the folder's lists and save it with `edit_folder`. They leave this value
/// as it is, so fetch the folder again to see the change.
impl Folder {
    /// Deletes the folder. The chats in it stay.
    pub async fn delete(&self, client: &Client) -> Result<()> {
        client.delete_folder(self.id).await
    }

    /// Saves the folder as it is now.
    pub async fn edit(&self, client: &Client) -> Result<()> {
        client.edit_folder(self).await
    }

    /// Adds the chat to the included chats, and takes it off the excluded ones.
    pub async fn include_chat(&self, client: &Client, chat_id: impl Into<PeerRef>) -> Result<()> {
        let id = client.peer_id(chat_id.into()).await?;
        let mut folder = self.clone();
        remove(&mut folder.excluded_chats, id);
        add(&mut folder.included_chats, id);
        client.edit_folder(&folder).await
    }

    /// Adds the chat to the excluded chats, and takes it off the included and pinned ones.
    pub async fn exclude_chat(&self, client: &Client, chat_id: impl Into<PeerRef>) -> Result<()> {
        let id = client.peer_id(chat_id.into()).await?;
        let mut folder = self.clone();
        remove(&mut folder.included_chats, id);
        remove(&mut folder.pinned_chats, id);
        add(&mut folder.excluded_chats, id);
        client.edit_folder(&folder).await
    }

    /// Pins the chat at the top of the folder, including it if it isn't yet.
    pub async fn pin_chat(&self, client: &Client, chat_id: impl Into<PeerRef>) -> Result<()> {
        let id = client.peer_id(chat_id.into()).await?;
        let mut folder = self.clone();
        remove(&mut folder.excluded_chats, id);
        add(&mut folder.included_chats, id);
        add(&mut folder.pinned_chats, id);
        client.edit_folder(&folder).await
    }

    /// Takes the chat off the pinned, included and excluded chats.
    pub async fn remove_chat(&self, client: &Client, chat_id: impl Into<PeerRef>) -> Result<()> {
        let id = client.peer_id(chat_id.into()).await?;
        let mut folder = self.clone();
        for list in [&mut folder.pinned_chats, &mut folder.included_chats, &mut folder.excluded_chats] {
            remove(list, id);
        }
        client.edit_folder(&folder).await
    }

    /// Changes the folder's tag color, or removes it when `color` is `None`.
    pub async fn update_color(&self, client: &Client, color: Option<i32>) -> Result<()> {
        let folder = Folder { color, ..self.clone() };
        client.edit_folder(&folder).await
    }

    /// Creates an invite link that shares the folder with the given chats.
    ///
    /// Awaiting returns the [`FolderInviteLink`](crate::types::FolderInviteLink).
    pub fn create_invite_link<'a>(
        &self,
        client: &'a Client,
        chat_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> CreateFolderInviteLink<'a> {
        client.create_folder_invite_link(self.id, chat_ids)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::Folder;

    fn client() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::recording();
        for id in [5, 6, 7] {
            let peer = types::InputPeerUser { user_id: id, access_hash: 1 };
            client.0.peers.lock().unwrap().insert(id, base::InputPeer::InputPeerUser(Box::new(peer)));
        }
        (client, rx)
    }

    async fn saved(client: &Client, rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, folder: &Folder) {
        let filter = Some(folder.write(client).await.unwrap());
        let expected = functions::messages::UpdateDialogFilter { id: folder.id, filter };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn folder_include_pin_remove_edit_lists() {
        let (client, mut rx) = client();
        let rx = &mut rx;
        let folder = Folder {
            id: 3,
            title: "Work".into(),
            pinned_chats: vec![5],
            included_chats: vec![5, 6],
            excluded_chats: vec![7],
            ..Folder::default()
        };
        let expect = |pinned: &[i64], included: &[i64], excluded: &[i64]| Folder {
            pinned_chats: pinned.to_vec(),
            included_chats: included.to_vec(),
            excluded_chats: excluded.to_vec(),
            ..folder.clone()
        };

        let _ = folder.include_chat(&client, 7).await;
        saved(&client, rx, &expect(&[5], &[5, 6, 7], &[])).await;
        let _ = folder.include_chat(&client, 6).await;
        saved(&client, rx, &expect(&[5], &[5, 6], &[7])).await;
        let _ = folder.exclude_chat(&client, 5).await;
        saved(&client, rx, &expect(&[], &[6], &[7, 5])).await;
        let _ = folder.pin_chat(&client, 7).await;
        saved(&client, rx, &expect(&[5, 7], &[5, 6, 7], &[])).await;
        let _ = folder.remove_chat(&client, 5).await;
        saved(&client, rx, &expect(&[], &[6], &[7])).await;
        let _ = folder.update_color(&client, Some(4)).await;
        saved(&client, rx, &Folder { color: Some(4), ..folder.clone() }).await;
        let _ = folder.edit(&client).await;
        saved(&client, rx, &folder).await;
        let _ = folder.delete(&client).await;
        let delete = functions::messages::UpdateDialogFilter { id: 3, filter: None };
        assert_eq!(rx.try_recv().unwrap(), delete.to_bytes());
    }
}
