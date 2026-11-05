use crate::Client;
use crate::errors::Result;
use crate::types::Animation;

impl Animation {
    /// Adds the animation to the account's saved GIFs.
    pub async fn add_to_gifs(&self, client: &Client) -> Result<()> {
        client.add_to_gifs(self).await
    }

    /// Removes the animation from the account's saved GIFs.
    pub async fn remove_from_gifs(&self, client: &Client) -> Result<()> {
        client.remove_from_gifs(self).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{Animation, Document};

    #[tokio::test]
    async fn animation_helpers_save_and_unsave_by_document() {
        let raw = base::Document::Document(Box::new(types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "video/mp4".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: Vec::new(),
        }));
        let gif = Animation::parse(Document::parse(&raw).unwrap(), None, None);
        let (client, mut rx) = Client::recording();
        let id = || {
            let document = types::InputDocument { id: 1, access_hash: 2, file_reference: vec![3] };
            base::InputDocument::InputDocument(Box::new(document))
        };
        let _ = gif.add_to_gifs(&client).await;
        assert_eq!(rx.try_recv().unwrap(), functions::messages::SaveGif { id: id(), unsave: false }.to_bytes());
        let _ = gif.remove_from_gifs(&client).await;
        assert_eq!(rx.try_recv().unwrap(), functions::messages::SaveGif { id: id(), unsave: true }.to_bytes());
    }
}
