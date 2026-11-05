use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

/// Request built by [`Client::set_upgraded_gift_colors`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetUpgradedGiftColors<'a> {
    client: &'a Client,
    upgraded_gift_colors_id: i64,
    for_profile: bool,
}

impl Client {
    /// Gives the account's name the colors of an upgraded gift it owns, by the upgraded gift's id; this needs
    /// Premium (`PREMIUM_ACCOUNT_REQUIRED`).
    pub fn set_upgraded_gift_colors(&self, upgraded_gift_colors_id: i64) -> SetUpgradedGiftColors<'_> {
        SetUpgradedGiftColors { client: self, upgraded_gift_colors_id, for_profile: false }
    }
}

impl SetUpgradedGiftColors<'_> {
    /// Colors the profile page instead of the name.
    pub fn for_profile(mut self) -> Self {
        self.for_profile = true;
        self
    }

    async fn send(self) -> Result<()> {
        let color = types::InputPeerColorCollectible { collectible_id: self.upgraded_gift_colors_id };
        let color = Some(base::PeerColor::InputPeerColorCollectible(color));
        self.client.invoke(&functions::account::UpdateColor { for_profile: self.for_profile, color }).await?;
        Ok(())
    }
}

into_future!(SetUpgradedGiftColors, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn sends_the_collectible_and_the_profile_flag() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.set_upgraded_gift_colors(77).await.unwrap();
        client.set_upgraded_gift_colors(77).for_profile().await.unwrap();
        for for_profile in [false, true] {
            let color = types::InputPeerColorCollectible { collectible_id: 77 };
            let color = Some(base::PeerColor::InputPeerColorCollectible(color));
            let expected = functions::account::UpdateColor { for_profile, color };
            assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        }

        let refused = Err(Error::Rpc(RpcError::new(400, "PREMIUM_ACCOUNT_REQUIRED")));
        let (client, _sent) = Client::scripted(vec![refused]);
        let result = client.set_upgraded_gift_colors(77).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "PREMIUM_ACCOUNT_REQUIRED"), "{result:?}");
    }
}
