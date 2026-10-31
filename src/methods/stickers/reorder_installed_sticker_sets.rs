use crate::Client;
use crate::enums::StickerType;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    pub async fn reorder_installed_sticker_sets(
        &self,
        sticker_type: StickerType,
        sticker_set_ids: &[i64],
    ) -> Result<()> {
        if sticker_set_ids.is_empty() {
            return Err(Error::InvalidArgument("reordering needs at least one sticker set id"));
        }
        let request = functions::messages::ReorderStickerSets {
            masks: sticker_type == StickerType::Mask,
            emojis: sticker_type == StickerType::CustomEmoji,
            order: sticker_set_ids.to_vec(),
        };
        self.invoke(&request).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::StickerType;
    use crate::errors::{Error, RpcError};
    use crate::raw::Serializable;
    use crate::raw::functions::messages::ReorderStickerSets;

    #[tokio::test]
    async fn each_sticker_type_sets_its_flag() {
        let (client, mut rx) = Client::recording();
        let cases = [
            (StickerType::Regular, false, false),
            (StickerType::Mask, true, false),
            (StickerType::CustomEmoji, false, true),
        ];
        for (kind, masks, emojis) in cases {
            let _ = client.reorder_installed_sticker_sets(kind, &[2, 1]).await;
            let expected = ReorderStickerSets { masks, emojis, order: vec![2, 1] };
            assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        }
    }

    #[tokio::test]
    async fn an_empty_order_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let result = client.reorder_installed_sticker_sets(StickerType::Regular, &[]).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "STICKERSET_INVALID")))]);
        let result = client.reorder_installed_sticker_sets(StickerType::Regular, &[1]).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "STICKERSET_INVALID"), "{result:?}");
    }
}
