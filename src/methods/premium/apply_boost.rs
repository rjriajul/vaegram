use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{MyBoost, PeerRef};

use super::get_my_boosts::my_boosts;

impl Client {
    pub async fn apply_boost(
        &self,
        chat_id: impl Into<PeerRef>,
        slots: impl IntoIterator<Item = i32>,
    ) -> Result<Vec<MyBoost>> {
        let slots: Vec<i32> = slots.into_iter().collect();
        if slots.is_empty() {
            return Err(Error::InvalidArgument("apply_boost needs at least one boost slot"));
        }
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::premium::ApplyBoost { slots: Some(slots), peer };
        Ok(my_boosts(self, self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;

    #[tokio::test]
    async fn no_slots_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.apply_boost(-1_000_000_000_001, []).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
