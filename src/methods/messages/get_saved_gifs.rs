use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Animation, Media};

impl Client {
    pub async fn get_saved_gifs(&self) -> Result<Vec<Animation>> {
        let gifs = match self.invoke(&functions::messages::GetSavedGifs { hash: 0 }).await? {
            base::messages::SavedGifs::SavedGifs(s) => s.gifs,
            base::messages::SavedGifs::SavedGifsNotModified => Vec::new(),
        };
        Ok(gifs
            .iter()
            .filter_map(|d| match Media::from_document(d)? {
                Media::Animation(a) => Some(a),
                _ => None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};

    fn document(id: i64, attributes: Vec<base::DocumentAttribute>) -> base::Document {
        base::Document::Document(Box::new(types::Document {
            id,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "video/mp4".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes,
        }))
    }

    #[tokio::test]
    async fn keeps_gifs_and_saves_them_by_document() {
        let gifs = vec![document(1, vec![base::DocumentAttribute::DocumentAttributeAnimated]), document(2, Vec::new())];
        let reply = base::messages::SavedGifs::SavedGifs(Box::new(types::messages::SavedGifs { hash: 9, gifs }));
        let (client, mut rx) = Client::scripted(vec![Ok(reply.to_bytes())]);

        let gifs = client.get_saved_gifs().await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), messages::GetSavedGifs { hash: 0 }.to_bytes());
        assert_eq!(gifs.iter().map(|g| g.id).collect::<Vec<_>>(), [1]);

        let id = || {
            let document = types::InputDocument { id: 1, access_hash: 2, file_reference: vec![3] };
            base::InputDocument::InputDocument(Box::new(document))
        };
        let _ = client.add_to_gifs(&gifs[0]).await;
        assert_eq!(rx.try_recv().unwrap(), messages::SaveGif { id: id(), unsave: false }.to_bytes());
        let _ = client.remove_from_gifs(&gifs[0]).await;
        assert_eq!(rx.try_recv().unwrap(), messages::SaveGif { id: id(), unsave: true }.to_bytes());
    }
}
