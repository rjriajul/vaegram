use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::GiftUpgradeVariants;

impl Client {
    pub async fn get_gift_upgrade_variants(&self, gift_id: i64) -> Result<GiftUpgradeVariants> {
        let request = functions::payments::GetStarGiftUpgradeAttributes { gift_id };
        let base::payments::StarGiftUpgradeAttributes::StarGiftUpgradeAttributes(reply) = self.invoke(&request).await?;
        Ok(GiftUpgradeVariants::parse(&reply.attributes))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn asks_for_the_gift_id() {
        let reply = types::payments::StarGiftUpgradeAttributes { attributes: Vec::new() };
        let reply = base::payments::StarGiftUpgradeAttributes::StarGiftUpgradeAttributes(Box::new(reply));
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let variants = client.get_gift_upgrade_variants(42).await.unwrap();
        assert!(variants.models.is_empty() && variants.symbols.is_empty() && variants.backdrops.is_empty());
        let request = functions::payments::GetStarGiftUpgradeAttributes { gift_id: 42 };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
