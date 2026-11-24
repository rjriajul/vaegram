use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{StickerSet, StickerSetRef};

impl Client {
    /// Loads a sticker set, with its stickers: by its short name, or by the [`StickerSetRef`] a
    /// [`Sticker`](crate::types::Sticker) or [`StickerSet`] carries.
    pub async fn get_sticker_set(&self, set: impl Into<StickerSetRef>) -> Result<StickerSet> {
        let request = functions::messages::GetStickerSet { stickerset: set.into().input(), hash: 0 };
        StickerSet::parse(self.invoke(&request).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::StickerSetRef;
    use crate::types::messages_and_media::sticker_set::tests::set;

    #[tokio::test]
    async fn loads_a_set_by_name_or_by_reference() {
        let reply = || Ok(set(false, false, None).to_bytes());
        let (client, mut sent) = Client::scripted_raw(vec![reply(), reply()]);
        let cats = client.get_sticker_set("cats_by_bot").await.unwrap();
        let name = types::InputStickerSetShortName { short_name: "cats_by_bot".into() };
        let stickerset = base::InputStickerSet::InputStickerSetShortName(Box::new(name));
        assert_eq!(sent.try_recv().unwrap(), messages::GetStickerSet { stickerset, hash: 0 }.to_bytes());

        let again = client.get_sticker_set(cats.set_ref()).await.unwrap();
        let id = types::InputStickerSetID { id: 10, access_hash: 11 };
        let stickerset = base::InputStickerSet::InputStickerSetID(Box::new(id));
        assert_eq!(sent.try_recv().unwrap(), messages::GetStickerSet { stickerset, hash: 0 }.to_bytes());
        assert_eq!((again.id, again.set_ref()), (10, StickerSetRef::from_id(10, 11)));
    }
}
