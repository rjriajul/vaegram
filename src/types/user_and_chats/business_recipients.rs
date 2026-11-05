use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};

/// Which chats a business feature applies to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BusinessRecipients {
    /// Includes chats that already exist.
    pub existing_chats: bool,
    /// Includes chats that are new.
    pub new_chats: bool,
    /// Includes chats with contacts.
    pub contacts: bool,
    /// Includes chats with people who aren't contacts.
    pub non_contacts: bool,
    /// Treats the flags above and `users` as chats to leave out instead of chats to include.
    pub exclude_selected: bool,
    /// Ids of users picked one by one.
    pub users: Vec<i64>,
    /// Ids of users a connected bot skips. Setting greeting and away messages fails with `InvalidArgument` if
    /// it isn't empty.
    pub exclude_users: Vec<i64>,
}

impl BusinessRecipients {
    pub(crate) fn parse(raw: base::BusinessRecipients) -> BusinessRecipients {
        let base::BusinessRecipients::BusinessRecipients(r) = raw;
        BusinessRecipients {
            existing_chats: r.existing_chats,
            new_chats: r.new_chats,
            contacts: r.contacts,
            non_contacts: r.non_contacts,
            exclude_selected: r.exclude_selected,
            users: r.users.unwrap_or_default(),
            exclude_users: Vec::new(),
        }
    }

    pub(crate) fn parse_bot(raw: base::BusinessBotRecipients) -> BusinessRecipients {
        let base::BusinessBotRecipients::BusinessBotRecipients(r) = raw;
        BusinessRecipients {
            existing_chats: r.existing_chats,
            new_chats: r.new_chats,
            contacts: r.contacts,
            non_contacts: r.non_contacts,
            exclude_selected: r.exclude_selected,
            users: r.users.unwrap_or_default(),
            exclude_users: r.exclude_users.unwrap_or_default(),
        }
    }

    /// Greeting and away messages have no exclusion list on the wire.
    pub(crate) async fn write(&self, client: &Client) -> Result<base::InputBusinessRecipients> {
        if !self.exclude_users.is_empty() {
            return Err(Error::InvalidArgument("these recipients take no excluded users"));
        }
        let r = types::InputBusinessRecipients {
            existing_chats: self.existing_chats,
            new_chats: self.new_chats,
            contacts: self.contacts,
            non_contacts: self.non_contacts,
            exclude_selected: self.exclude_selected,
            users: input_users(client, &self.users).await?,
        };
        Ok(base::InputBusinessRecipients::InputBusinessRecipients(Box::new(r)))
    }

    pub(crate) async fn write_bot(&self, client: &Client) -> Result<base::InputBusinessBotRecipients> {
        let r = types::InputBusinessBotRecipients {
            existing_chats: self.existing_chats,
            new_chats: self.new_chats,
            contacts: self.contacts,
            non_contacts: self.non_contacts,
            exclude_selected: self.exclude_selected,
            users: input_users(client, &self.users).await?,
            exclude_users: input_users(client, &self.exclude_users).await?,
        };
        Ok(base::InputBusinessBotRecipients::InputBusinessBotRecipients(Box::new(r)))
    }
}

async fn input_users(client: &Client, ids: &[i64]) -> Result<Option<Vec<base::InputUser>>> {
    if ids.is_empty() {
        return Ok(None);
    }
    client.resolve_users(ids.iter().copied()).await.map(Some)
}
