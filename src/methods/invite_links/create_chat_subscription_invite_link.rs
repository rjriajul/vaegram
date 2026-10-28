use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{ChatInviteLink, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct CreateChatSubscriptionInviteLink<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    subscription_period: i32,
    subscription_price: i64,
    name: Option<String>,
}

impl Client {
    /// Channel link that charges `subscription_price` Stars every `subscription_period` seconds
    /// (Telegram only accepts 30 days).
    pub fn create_chat_subscription_invite_link(
        &self,
        chat_id: impl Into<PeerRef>,
        subscription_period: i32,
        subscription_price: i64,
    ) -> CreateChatSubscriptionInviteLink<'_> {
        CreateChatSubscriptionInviteLink {
            client: self,
            chat_id: chat_id.into(),
            subscription_period,
            subscription_price,
            name: None,
        }
    }
}

impl CreateChatSubscriptionInviteLink<'_> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    async fn send(self) -> Result<ChatInviteLink> {
        let pricing =
            types::StarsSubscriptionPricing { period: self.subscription_period, amount: self.subscription_price };
        let request = functions::messages::ExportChatInvite {
            legacy_revoke_permanent: false,
            request_needed: false,
            peer: self.client.resolve_peer(self.chat_id).await?,
            expire_date: None,
            usage_limit: None,
            title: self.name,
            subscription_pricing: Some(base::StarsSubscriptionPricing::StarsSubscriptionPricing(Box::new(pricing))),
        };
        ChatInviteLink::parse(self.client.invoke(&request).await?, &Entities::default())
    }
}

into_future!(CreateChatSubscriptionInviteLink, ChatInviteLink);
