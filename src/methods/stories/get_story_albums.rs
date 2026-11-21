use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, StoryAlbum};

impl Client {
    /// Lists the story albums on a user's or channel's profile.
    pub async fn get_story_albums(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<StoryAlbum>> {
        let peer = self.resolve_peer(chat_id).await?;
        Ok(match self.invoke(&functions::stories::GetAlbums { peer, hash: 0 }).await? {
            base::stories::Albums::Albums(list) => list.albums.into_iter().map(StoryAlbum::parse).collect(),
            base::stories::Albums::AlbumsNotModified => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::stories;
    use crate::raw::{Serializable, base, types};
    use crate::types::StoryAlbum;

    #[tokio::test]
    async fn lists_the_albums() {
        let album = |album_id, title: &str| {
            let album = types::StoryAlbum { album_id, title: title.into(), icon_photo: None, icon_video: None };
            base::StoryAlbum::StoryAlbum(Box::new(album))
        };
        let list = types::stories::Albums { hash: 5, albums: vec![album(1, "Trips"), album(2, "Pets")] };
        let reply = base::stories::Albums::Albums(Box::new(list)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);

        let found = client.get_story_albums("me").await.unwrap();
        let expected = |id, title: &str| StoryAlbum { id, title: title.into(), icon_photo: None, icon_video: None };
        assert_eq!(found, vec![expected(1, "Trips"), expected(2, "Pets")]);
        let request = stories::GetAlbums { peer: base::InputPeer::InputPeerSelf, hash: 0 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn not_modified_is_an_empty_list() {
        let reply = base::stories::Albums::AlbumsNotModified.to_bytes();
        let (client, _sent) = Client::scripted(vec![Ok(reply)]);
        assert!(client.get_story_albums("me").await.unwrap().is_empty());
    }
}
