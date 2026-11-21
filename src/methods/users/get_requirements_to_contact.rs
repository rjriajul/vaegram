use crate::Client;
use crate::enums::ContactRequirement;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Returns what each user asks of you before you may write to them first, one slot per user in the order given.
    ///
    /// Use it before a first message to learn that a user takes first messages only from Premium accounts, or for
    /// Stars, instead of getting `PRIVACY_PREMIUM_REQUIRED` or a payment error back from the send.
    ///
    /// Fails with `Error::InvalidArgument` if `user_ids` is empty.
    pub async fn get_requirements_to_contact(
        &self,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<Vec<ContactRequirement>> {
        let id = self.resolve_users(user_ids).await?;
        if id.is_empty() {
            return Err(Error::InvalidArgument("get_requirements_to_contact needs at least one user"));
        }
        let requirements = self.invoke(&functions::users::GetRequirementsToContact { id }).await?;
        Ok(requirements.into_iter().map(ContactRequirement::parse).collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ContactRequirement;
    use crate::errors::Error;
    use crate::raw::functions::users;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn returns_one_requirement_per_user() {
        let reply = vec![
            base::RequirementToContact::RequirementToContactEmpty,
            base::RequirementToContact::RequirementToContactPremium,
            base::RequirementToContact::RequirementToContactPaidMessages(types::RequirementToContactPaidMessages {
                stars_amount: 25,
            }),
        ];
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let mut id = Vec::new();
        for user_id in [9, 10, 11] {
            let peer = types::InputPeerUser { user_id, access_hash: user_id + 100 };
            client.0.peers.lock().unwrap().insert(user_id, base::InputPeer::InputPeerUser(Box::new(peer)));
            id.push(base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: user_id + 100 })));
        }

        let found = client.get_requirements_to_contact([9, 10, 11]).await.unwrap();
        let expected = [
            ContactRequirement::Unrestricted,
            ContactRequirement::Premium,
            ContactRequirement::PaidMessages { stars: 25 },
        ];
        assert_eq!(found, expected);
        assert_eq!(sent.try_recv().unwrap(), users::GetRequirementsToContact { id }.to_bytes());
    }

    #[tokio::test]
    async fn no_users_is_refused_without_a_request() {
        let (client, mut sent) = Client::recording();
        let result = client.get_requirements_to_contact(Vec::<i64>::new()).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }
}
