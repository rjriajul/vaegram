use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StarAmount;

impl Client {
    /// The account's TON balance, in nanotons.
    pub async fn get_ton_balance(&self) -> Result<StarAmount> {
        let request = functions::payments::GetStarsStatus { ton: true, peer: base::InputPeer::InputPeerSelf };
        let base::payments::StarsStatus::StarsStatus(status) = self.invoke(&request).await?;
        Ok(StarAmount::parse(&status.balance))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::StarAmount;

    #[tokio::test]
    async fn reads_the_ton_balance() {
        let status = types::payments::StarsStatus {
            balance: base::StarsAmount::StarsTonAmount(types::StarsTonAmount { amount: 5 }),
            subscriptions: None,
            subscriptions_next_offset: None,
            subscriptions_missing_balance: None,
            history: None,
            next_offset: None,
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = base::payments::StarsStatus::StarsStatus(Box::new(status)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);
        assert_eq!(client.get_ton_balance().await.unwrap(), StarAmount { amount: 5, nanostar_amount: 0, ton: true });
        let request = functions::payments::GetStarsStatus { ton: true, peer: base::InputPeer::InputPeerSelf };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
