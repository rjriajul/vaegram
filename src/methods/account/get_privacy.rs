use crate::Client;
use crate::enums::PrivacyKey;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PrivacyRule;

impl Client {
    pub async fn get_privacy(&self, key: PrivacyKey) -> Result<Vec<PrivacyRule>> {
        let rules = self.invoke(&functions::account::GetPrivacy { key: key.write() }).await?;
        Ok(PrivacyRule::parse_all(self, rules))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::PrivacyKey;
    use crate::raw::{Deserializable, Serializable, base, types};
    use crate::types::PrivacyRule;

    // user#b1b8cc83 with only access_hash set.
    fn user(id: i64, access_hash: i64) -> base::User {
        let mut bytes = Vec::new();
        for word in [0xb1b8cc83u32, 1, 0] {
            bytes.extend(word.to_le_bytes());
        }
        bytes.extend(id.to_le_bytes());
        bytes.extend(access_hash.to_le_bytes());
        base::User::from_bytes(&bytes).unwrap()
    }

    #[tokio::test]
    async fn listed_users_can_be_written_back() {
        let rule = types::PrivacyValueAllowUsers { users: vec![7] };
        let rule = base::PrivacyRule::PrivacyValueAllowUsers(Box::new(rule));
        let reply = types::account::PrivacyRules { rules: vec![rule], chats: Vec::new(), users: vec![user(7, 99)] };
        let reply = base::account::PrivacyRules::PrivacyRules(Box::new(reply));
        let (client, _sent) = Client::scripted(vec![Ok(reply.to_bytes())]);

        let rules = client.get_privacy(PrivacyKey::PhoneNumber).await.unwrap();
        assert_eq!(rules, [PrivacyRule::AllowUsers(vec![7])]);
        let cached = types::InputPeerUser { user_id: 7, access_hash: 99 };
        let cached = base::InputPeer::InputPeerUser(Box::new(cached));
        assert_eq!(client.0.peers.lock().unwrap().get(&7), Some(&cached));
    }
}
