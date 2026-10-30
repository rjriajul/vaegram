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
}
