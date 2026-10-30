use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::utils::get_channel_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyRule {
    AllowAll,
    AllowContacts,
    AllowCloseFriends,
    AllowPremium,
    AllowBots,
    AllowUsers(Vec<i64>),
    AllowChatMembers(Vec<i64>),
    DisallowAll,
    DisallowContacts,
    DisallowBots,
    DisallowUsers(Vec<i64>),
    DisallowChatMembers(Vec<i64>),
}

impl PrivacyRule {
    /// Caches the listed users and chats, so the rules can be passed back to `set_privacy` as they are.
    pub(crate) fn parse_all(client: &Client, raw: base::account::PrivacyRules) -> Vec<PrivacyRule> {
        let base::account::PrivacyRules::PrivacyRules(r) = raw;
        let entities = Entities::new(client, r.users, r.chats);
        r.rules.into_iter().map(|rule| PrivacyRule::parse(rule, &entities)).collect()
    }

    // Telegram sends bare chat ids here, so only the listed chats tell a basic group from a channel.
    fn parse(rule: base::PrivacyRule, entities: &Entities) -> PrivacyRule {
        use base::PrivacyRule as R;
        let chat_ids = |ids: Vec<i64>| {
            let signed = |id: i64| {
                let channel = get_channel_id(id);
                if entities.chats.contains_key(&channel) { channel } else { id.wrapping_neg() }
            };
            ids.into_iter().map(signed).collect()
        };
        match rule {
            R::PrivacyValueAllowAll => PrivacyRule::AllowAll,
            R::PrivacyValueAllowContacts => PrivacyRule::AllowContacts,
            R::PrivacyValueAllowCloseFriends => PrivacyRule::AllowCloseFriends,
            R::PrivacyValueAllowPremium => PrivacyRule::AllowPremium,
            R::PrivacyValueAllowBots => PrivacyRule::AllowBots,
            R::PrivacyValueAllowUsers(r) => PrivacyRule::AllowUsers(r.users),
            R::PrivacyValueAllowChatParticipants(r) => PrivacyRule::AllowChatMembers(chat_ids(r.chats)),
            R::PrivacyValueDisallowAll => PrivacyRule::DisallowAll,
            R::PrivacyValueDisallowContacts => PrivacyRule::DisallowContacts,
            R::PrivacyValueDisallowBots => PrivacyRule::DisallowBots,
            R::PrivacyValueDisallowUsers(r) => PrivacyRule::DisallowUsers(r.users),
            R::PrivacyValueDisallowChatParticipants(r) => PrivacyRule::DisallowChatMembers(chat_ids(r.chats)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PrivacyRule;
    use crate::Client;
    use crate::raw::{base, types};

    fn channel(id: i64) -> base::Chat {
        base::Chat::ChannelForbidden(Box::new(types::ChannelForbidden {
            broadcast: true,
            megagroup: false,
            monoforum: false,
            id,
            access_hash: 1,
            title: String::new(),
            until_date: None,
        }))
    }

    #[test]
    fn chat_ids_get_their_signed_form() {
        let rule = types::PrivacyValueAllowChatParticipants { chats: vec![5, 7] };
        let rule = base::PrivacyRule::PrivacyValueAllowChatParticipants(Box::new(rule));
        let raw = types::account::PrivacyRules { rules: vec![rule], chats: vec![channel(7)], users: Vec::new() };
        let raw = base::account::PrivacyRules::PrivacyRules(Box::new(raw));
        let rules = PrivacyRule::parse_all(&Client::offline(), raw);
        assert_eq!(rules, [PrivacyRule::AllowChatMembers(vec![-5, -1_000_000_000_007])]);
    }
}
