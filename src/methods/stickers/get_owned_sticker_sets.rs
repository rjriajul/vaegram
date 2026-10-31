use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StickerSet;

#[must_use = "a request does nothing until it is awaited"]
pub struct GetOwnedStickerSets<'a> {
    client: &'a Client,
    request: functions::messages::GetMyStickers,
}

impl Client {
    pub fn get_owned_sticker_sets(&self) -> GetOwnedStickerSets<'_> {
        GetOwnedStickerSets { client: self, request: functions::messages::GetMyStickers { offset_id: 0, limit: 100 } }
    }
}

impl GetOwnedStickerSets<'_> {
    /// Continues after the set with this id, the last one of the previous page.
    pub fn offset_id(mut self, set_id: i64) -> Self {
        self.request.offset_id = set_id;
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.request.limit = limit;
        self
    }

    async fn send(self) -> Result<Vec<StickerSet>> {
        let base::messages::MyStickers::MyStickers(page) = self.client.invoke(&self.request).await?;
        Ok(page.sets.iter().map(StickerSet::parse_covered).collect())
    }
}

into_future!(GetOwnedStickerSets, Vec<StickerSet>);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::StickerType;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::messages::GetMyStickers;
    use crate::raw::{Serializable, base, types};

    fn reply(sets: Vec<base::StickerSetCovered>) -> Vec<u8> {
        let count = sets.len() as i32;
        base::messages::MyStickers::MyStickers(Box::new(types::messages::MyStickers { count, sets })).to_bytes()
    }

    fn covered() -> base::StickerSetCovered {
        let set = types::StickerSet {
            archived: false,
            official: false,
            masks: false,
            emojis: true,
            text_color: false,
            channel_emoji_status: false,
            creator: true,
            installed_date: None,
            id: 11,
            access_hash: 12,
            title: "Mine".into(),
            short_name: "mine_by_bot".into(),
            thumbs: None,
            thumb_dc_id: None,
            thumb_version: None,
            thumb_document_id: None,
            count: 1,
            hash: 0,
        };
        let cover = base::Document::DocumentEmpty(types::DocumentEmpty { id: 1 });
        let covered = types::StickerSetCovered { set: base::StickerSet::StickerSet(Box::new(set)), cover };
        base::StickerSetCovered::StickerSetCovered(Box::new(covered))
    }

    #[tokio::test]
    async fn pages_through_the_sets_the_user_created() {
        let (client, mut rx) = Client::scripted(vec![Ok(reply(vec![covered()])), Ok(reply(Vec::new()))]);
        let sets = client.get_owned_sticker_sets().await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), GetMyStickers { offset_id: 0, limit: 100 }.to_bytes());
        assert_eq!(sets.len(), 1);
        let set = &sets[0];
        assert_eq!((set.id, set.name.as_str(), set.title.as_str()), (11, "mine_by_bot", "Mine"));
        assert_eq!(set.sticker_type, StickerType::CustomEmoji);

        // The account owns no more sets past this one.
        assert!(client.get_owned_sticker_sets().offset_id(77).limit(10).await.unwrap().is_empty());
        assert_eq!(rx.try_recv().unwrap(), GetMyStickers { offset_id: 77, limit: 10 }.to_bytes());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "OFFSET_INVALID")))]);
        let result = client.get_owned_sticker_sets().await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "OFFSET_INVALID"), "{result:?}");
    }
}
