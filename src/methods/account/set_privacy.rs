use crate::Client;
use crate::enums::PrivacyKey;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::PrivacyRule;
use crate::utils::get_peer;

impl Client {
    pub async fn set_privacy(&self, key: PrivacyKey, rules: &[PrivacyRule]) -> Result<Vec<PrivacyRule>> {
        let mut input = Vec::with_capacity(rules.len());
        for rule in rules {
            use base::InputPrivacyRule as I;
            input.push(match rule {
                PrivacyRule::AllowAll => I::InputPrivacyValueAllowAll,
                PrivacyRule::AllowContacts => I::InputPrivacyValueAllowContacts,
                PrivacyRule::AllowCloseFriends => I::InputPrivacyValueAllowCloseFriends,
                PrivacyRule::AllowPremium => I::InputPrivacyValueAllowPremium,
                PrivacyRule::AllowBots => I::InputPrivacyValueAllowBots,
                PrivacyRule::AllowUsers(ids) => {
                    I::InputPrivacyValueAllowUsers(Box::new(types::InputPrivacyValueAllowUsers {
                        users: self.input_users(ids).await?,
                    }))
                }
                PrivacyRule::AllowChatMembers(ids) => {
                    I::InputPrivacyValueAllowChatParticipants(Box::new(types::InputPrivacyValueAllowChatParticipants {
                        chats: bare_chat_ids(ids)?,
                    }))
                }
                PrivacyRule::DisallowAll => I::InputPrivacyValueDisallowAll,
                PrivacyRule::DisallowContacts => I::InputPrivacyValueDisallowContacts,
                PrivacyRule::DisallowBots => I::InputPrivacyValueDisallowBots,
                PrivacyRule::DisallowUsers(ids) => {
                    I::InputPrivacyValueDisallowUsers(Box::new(types::InputPrivacyValueDisallowUsers {
                        users: self.input_users(ids).await?,
                    }))
                }
                PrivacyRule::DisallowChatMembers(ids) => I::InputPrivacyValueDisallowChatParticipants(Box::new(
                    types::InputPrivacyValueDisallowChatParticipants { chats: bare_chat_ids(ids)? },
                )),
            });
        }
        let rules = self.invoke(&functions::account::SetPrivacy { key: key.write(), rules: input }).await?;
        Ok(PrivacyRule::parse_all(self, rules))
    }

    async fn input_users(&self, ids: &[i64]) -> Result<Vec<base::InputUser>> {
        let mut users = Vec::with_capacity(ids.len());
        for &id in ids {
            users.push(self.resolve_user(&id.into()).await?);
        }
        Ok(users)
    }
}

fn bare_chat_ids(ids: &[i64]) -> Result<Vec<i64>> {
    let bare = |&id: &i64| match get_peer(id) {
        base::Peer::PeerChat(c) if c.chat_id > 0 => Ok(c.chat_id),
        base::Peer::PeerChannel(c) if c.channel_id > 0 => Ok(c.channel_id),
        _ => Err(Error::InvalidArgument("chat member rules take group and channel ids")),
    };
    ids.iter().map(bare).collect()
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::PrivacyKey;
    use crate::errors::{Error, RpcError};
    use crate::types::PrivacyRule;

    #[tokio::test]
    async fn chat_member_rules_refuse_user_ids_before_sending() {
        let refused = || Err(Error::Rpc(RpcError::new(400, "PRIVACY_VALUE_INVALID")));
        let (client, mut sent) = Client::scripted(vec![refused()]);
        let result = client.set_privacy(PrivacyKey::Bio, &[PrivacyRule::AllowChatMembers(vec![7])]).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_out_of_range_chat_id_is_an_error_not_a_panic() {
        let refused = || Err(Error::Rpc(RpcError::new(400, "PRIVACY_VALUE_INVALID")));
        let (client, _sent) = Client::scripted(vec![refused()]);
        let result = client.set_privacy(PrivacyKey::Bio, &[PrivacyRule::DisallowChatMembers(vec![i64::MIN])]).await;
        assert!(result.is_err());
    }

    #[test]
    fn bare_chat_ids_strip_the_sign_and_channel_offset() {
        assert_eq!(super::bare_chat_ids(&[-5, -1_000_000_000_007]).unwrap(), vec![5, 7]);
    }
}
