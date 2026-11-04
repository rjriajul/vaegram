use crate::types::PrivacyRule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoriesPrivacyRules {
    Public {
        except: Vec<i64>,
    },
    Contacts {
        except: Vec<i64>,
    },
    CloseFriends,
    /// Negative ids are groups: their members can see the story.
    SelectedUsers(Vec<i64>),
}

impl Default for StoriesPrivacyRules {
    fn default() -> Self {
        StoriesPrivacyRules::Public { except: Vec::new() }
    }
}

impl StoriesPrivacyRules {
    pub(crate) fn rules(&self) -> Vec<PrivacyRule> {
        let (allow, except) = match self {
            StoriesPrivacyRules::Public { except } => (PrivacyRule::AllowAll, except),
            StoriesPrivacyRules::Contacts { except } => (PrivacyRule::AllowContacts, except),
            StoriesPrivacyRules::CloseFriends => return vec![PrivacyRule::AllowCloseFriends],
            StoriesPrivacyRules::SelectedUsers(ids) => {
                let (users, chats): (Vec<i64>, Vec<i64>) = ids.iter().partition(|&&id| id > 0);
                let mut rules = vec![PrivacyRule::AllowUsers(users)];
                if !chats.is_empty() {
                    rules.push(PrivacyRule::AllowChatMembers(chats));
                }
                return rules;
            }
        };
        let mut rules = vec![allow];
        if !except.is_empty() {
            rules.push(PrivacyRule::DisallowUsers(except.clone()));
        }
        rules
    }

    /// The audience a story's privacy rules describe, or `None` when they are not one `rules` produces.
    pub(crate) fn from_rules(rules: &[PrivacyRule]) -> Option<StoriesPrivacyRules> {
        let except = |rest: &[PrivacyRule]| match rest {
            [] => Some(Vec::new()),
            [PrivacyRule::DisallowUsers(users)] => Some(users.clone()),
            _ => None,
        };
        match rules {
            [PrivacyRule::AllowAll, rest @ ..] => Some(StoriesPrivacyRules::Public { except: except(rest)? }),
            [PrivacyRule::AllowContacts, rest @ ..] => Some(StoriesPrivacyRules::Contacts { except: except(rest)? }),
            [PrivacyRule::AllowCloseFriends] => Some(StoriesPrivacyRules::CloseFriends),
            [PrivacyRule::AllowUsers(users)] => Some(StoriesPrivacyRules::SelectedUsers(users.clone())),
            // Telegram may leave out the empty user rule when only groups are allowed.
            [PrivacyRule::AllowChatMembers(chats)] => Some(StoriesPrivacyRules::SelectedUsers(chats.clone())),
            [PrivacyRule::AllowUsers(users), PrivacyRule::AllowChatMembers(chats)] => {
                Some(StoriesPrivacyRules::SelectedUsers(users.iter().chain(chats).copied().collect()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StoriesPrivacyRules;
    use crate::types::PrivacyRule;

    #[test]
    fn lowers_each_audience_to_privacy_rules() {
        assert_eq!(StoriesPrivacyRules::default().rules(), [PrivacyRule::AllowAll]);
        assert_eq!(
            StoriesPrivacyRules::Contacts { except: vec![5] }.rules(),
            [PrivacyRule::AllowContacts, PrivacyRule::DisallowUsers(vec![5])]
        );
        assert_eq!(StoriesPrivacyRules::CloseFriends.rules(), [PrivacyRule::AllowCloseFriends]);
        assert_eq!(
            StoriesPrivacyRules::SelectedUsers(vec![6, -42, -1_000_000_001_234]).rules(),
            [PrivacyRule::AllowUsers(vec![6]), PrivacyRule::AllowChatMembers(vec![-42, -1_000_000_001_234])]
        );
    }

    #[test]
    fn rules_parse_back() {
        for audience in [
            StoriesPrivacyRules::default(),
            StoriesPrivacyRules::Public { except: vec![3, 4] },
            StoriesPrivacyRules::Contacts { except: Vec::new() },
            StoriesPrivacyRules::Contacts { except: vec![5] },
            StoriesPrivacyRules::CloseFriends,
            StoriesPrivacyRules::SelectedUsers(vec![6, 7]),
            StoriesPrivacyRules::SelectedUsers(vec![6, -42, -1_000_000_001_234]),
        ] {
            assert_eq!(StoriesPrivacyRules::from_rules(&audience.rules()), Some(audience));
        }
    }

    #[test]
    fn chat_only_rule_selects_those_chats() {
        let rules = [PrivacyRule::AllowChatMembers(vec![-42])];
        assert_eq!(StoriesPrivacyRules::from_rules(&rules), Some(StoriesPrivacyRules::SelectedUsers(vec![-42])));
    }

    #[test]
    fn unrepresentable_rules_give_none() {
        assert_eq!(StoriesPrivacyRules::from_rules(&[]), None);
        assert_eq!(StoriesPrivacyRules::from_rules(&[PrivacyRule::DisallowAll]), None);
        assert_eq!(StoriesPrivacyRules::from_rules(&[PrivacyRule::AllowPremium]), None);
        let rules = [PrivacyRule::AllowAll, PrivacyRule::AllowContacts];
        assert_eq!(StoriesPrivacyRules::from_rules(&rules), None);
        let rules = [PrivacyRule::AllowCloseFriends, PrivacyRule::DisallowUsers(vec![1])];
        assert_eq!(StoriesPrivacyRules::from_rules(&rules), None);
    }
}
