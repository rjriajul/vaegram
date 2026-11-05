use crate::types::PrivacyRule;

/// Who can see a story. The default is [`Public`](Self::Public) with no exceptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoriesPrivacyRules {
    /// Everyone except the listed user ids.
    Public {
        /// User ids that can't see the story.
        except: Vec<i64>,
    },
    /// Your contacts except the listed user ids.
    Contacts {
        /// User ids that can't see the story.
        except: Vec<i64>,
    },
    /// Only your close friends list.
    CloseFriends,
    /// Only the listed users. Negative ids are groups, and their members can see the story.
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
        // Exclusions come first: the official client sends them before the allow rule.
        let mut rules = Vec::new();
        if !except.is_empty() {
            rules.push(PrivacyRule::DisallowUsers(except.clone()));
        }
        rules.push(allow);
        rules
    }

    /// The audience a story's privacy rules describe, or `None` when they are not one `rules` produces.
    pub(crate) fn from_rules(rules: &[PrivacyRule]) -> Option<StoriesPrivacyRules> {
        // Either order is read: the exclusions may come before or after the allow rule.
        let (allow, except) = match rules {
            [PrivacyRule::DisallowUsers(users), allow] => (std::slice::from_ref(allow), users.clone()),
            [allow, PrivacyRule::DisallowUsers(users)] => (std::slice::from_ref(allow), users.clone()),
            _ => (rules, Vec::new()),
        };
        match allow {
            [PrivacyRule::AllowAll] => Some(StoriesPrivacyRules::Public { except }),
            [PrivacyRule::AllowContacts] => Some(StoriesPrivacyRules::Contacts { except }),
            // Exclusions only go with the two open audiences.
            _ if !except.is_empty() => None,
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
            [PrivacyRule::DisallowUsers(vec![5]), PrivacyRule::AllowContacts]
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
    fn reads_exclusions_in_either_order() {
        let before = [PrivacyRule::DisallowUsers(vec![5]), PrivacyRule::AllowContacts];
        let after = [PrivacyRule::AllowContacts, PrivacyRule::DisallowUsers(vec![5])];
        let expected = Some(StoriesPrivacyRules::Contacts { except: vec![5] });
        assert_eq!(StoriesPrivacyRules::from_rules(&before), expected);
        assert_eq!(StoriesPrivacyRules::from_rules(&after), expected);
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
