use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::SubscriptionState;
use crate::raw::types;
use crate::types::User;

#[derive(Clone)]
pub struct BotSubscriptionUpdated {
    pub user: User,
    pub invoice_payload: String,
    pub state: SubscriptionState,
    client: Client,
}

impl BotSubscriptionUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotStarsSubscription, entities: &Entities) -> Option<Self> {
        let state = match (raw.payment_failed, raw.canceled, raw.restored) {
            (true, ..) => SubscriptionState::Failed,
            (_, true, _) => SubscriptionState::Canceled,
            (_, _, true) => SubscriptionState::Active,
            _ => return None,
        };
        Some(BotSubscriptionUpdated {
            user: User::parse(raw.user_id, entities),
            invoice_payload: String::from_utf8_lossy(&raw.payload).into_owned(),
            state,
            client: client.clone(),
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl fmt::Debug for BotSubscriptionUpdated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BotSubscriptionUpdated")
            .field("user", &self.user)
            .field("invoice_payload", &self.invoice_payload)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::BotSubscriptionUpdated;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::enums::SubscriptionState;
    use crate::raw::types;

    fn state(canceled: bool, payment_failed: bool, restored: bool) -> Option<SubscriptionState> {
        let raw = types::UpdateBotStarsSubscription {
            canceled,
            payment_failed,
            restored,
            user_id: 7,
            payload: b"plan".to_vec(),
            qts: 1,
        };
        BotSubscriptionUpdated::parse(&Client::offline(), &raw, &Entities::default()).map(|s| s.state)
    }

    #[test]
    fn maps_flags_to_state() {
        assert_eq!(state(true, false, false), Some(SubscriptionState::Canceled));
        assert_eq!(state(false, true, false), Some(SubscriptionState::Failed));
        assert_eq!(state(false, false, true), Some(SubscriptionState::Active));
        assert_eq!(state(false, false, false), None);
    }
}
