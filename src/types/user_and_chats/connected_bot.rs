use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{BusinessBotRights, BusinessRecipients, User};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedBot {
    pub bot: User,
    pub recipients: BusinessRecipients,
    pub rights: BusinessBotRights,
    pub device: Option<String>,
    pub date: Option<i32>,
    pub location: Option<String>,
}

impl ConnectedBot {
    pub(crate) fn parse(raw: base::ConnectedBot, entities: &Entities) -> ConnectedBot {
        let base::ConnectedBot::ConnectedBot(b) = raw;
        let types::ConnectedBot { bot_id, recipients, rights, device, date, location } = *b;
        let (recipients, rights) = (BusinessRecipients::parse_bot(recipients), BusinessBotRights::parse(&rights));
        ConnectedBot { bot: User::parse(bot_id, entities), recipients, rights, device, date, location }
    }
}

#[cfg(test)]
mod tests {
    use super::ConnectedBot;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::BusinessBotRights;

    #[test]
    fn keeps_recipients_and_tolerates_a_missing_bot_user() {
        let recipients = types::BusinessBotRecipients {
            existing_chats: false,
            new_chats: true,
            contacts: false,
            non_contacts: true,
            exclude_selected: true,
            users: None,
            exclude_users: Some(vec![8, 9]),
        };
        let rights = BusinessBotRights { can_reply: true, ..Default::default() }.write();
        let raw = types::ConnectedBot {
            bot_id: 42,
            recipients: base::BusinessBotRecipients::BusinessBotRecipients(Box::new(recipients)),
            rights,
            device: Some("phone".into()),
            date: None,
            location: None,
        };
        let bot = ConnectedBot::parse(base::ConnectedBot::ConnectedBot(Box::new(raw)), &Entities::default());
        let r = &bot.recipients;
        assert_eq!((bot.bot.id, bot.rights.can_reply, bot.rights.can_read_messages), (42, true, false));
        assert_eq!((r.existing_chats, r.new_chats, r.contacts, r.non_contacts), (false, true, false, true));
        assert_eq!((r.exclude_selected, r.users.as_slice(), r.exclude_users.as_slice()), (true, &[][..], &[8, 9][..]));
    }
}
