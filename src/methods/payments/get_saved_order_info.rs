use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::OrderInfo;

impl Client {
    /// The contact and shipping details saved by an earlier order, if any.
    pub async fn get_saved_order_info(&self) -> Result<Option<OrderInfo>> {
        let base::payments::SavedInfo::SavedInfo(saved) = self.invoke(&functions::payments::GetSavedInfo {}).await?;
        Ok(saved.saved_info.as_ref().map(OrderInfo::parse))
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::OrderInfo;

    fn saved(info: Option<OrderInfo>) -> Vec<u8> {
        let saved = types::payments::SavedInfo { has_saved_credentials: true, saved_info: info.map(OrderInfo::write) };
        base::payments::SavedInfo::SavedInfo(Box::new(saved)).to_bytes()
    }

    #[tokio::test]
    async fn saved_info_reads_and_clears() {
        let info = OrderInfo::new().name("Ann").email("a@b.c");
        let (client, mut sent) = client(vec![saved(Some(info.clone())), saved(None), true.to_bytes(), true.to_bytes()]);
        assert_eq!(client.get_saved_order_info().await.unwrap(), Some(info));
        assert_eq!(sent.try_recv().unwrap(), functions::payments::GetSavedInfo {}.to_bytes());
        assert_eq!(client.get_saved_order_info().await.unwrap(), None);
        sent.try_recv().unwrap();

        client.delete_saved_order_info().await.unwrap();
        let request = functions::payments::ClearSavedInfo { credentials: false, info: true };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        client.delete_saved_credentials().await.unwrap();
        let request = functions::payments::ClearSavedInfo { credentials: true, info: false };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert!(sent.try_recv().is_err());
    }
}
