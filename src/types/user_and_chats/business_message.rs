use crate::enums::BusinessAwayMessageSchedule;
use crate::raw::base;
use crate::types::BusinessRecipients;

/// The quick reply a business account sends to new or long-quiet chats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessGreetingMessage {
    /// Id of the quick reply that is sent.
    pub shortcut_id: i32,
    /// Chats the message is sent to.
    pub recipients: BusinessRecipients,
    /// Days a chat must be quiet before it's greeted again.
    pub no_activity_days: i32,
}

/// The quick reply a business account sends while it is away.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessAwayMessage {
    /// Id of the quick reply that is sent.
    pub shortcut_id: i32,
    /// Chats the message is sent to.
    pub recipients: BusinessRecipients,
    /// When the message is sent.
    pub schedule: BusinessAwayMessageSchedule,
    /// `true` to send the message only while the account is offline.
    pub offline_only: bool,
}

impl BusinessGreetingMessage {
    pub(crate) fn parse(raw: base::BusinessGreetingMessage) -> BusinessGreetingMessage {
        let base::BusinessGreetingMessage::BusinessGreetingMessage(m) = raw;
        let m = *m;
        BusinessGreetingMessage {
            shortcut_id: m.shortcut_id,
            recipients: BusinessRecipients::parse(m.recipients),
            no_activity_days: m.no_activity_days,
        }
    }
}

impl BusinessAwayMessage {
    pub(crate) fn parse(raw: base::BusinessAwayMessage) -> BusinessAwayMessage {
        let base::BusinessAwayMessage::BusinessAwayMessage(m) = raw;
        let m = *m;
        BusinessAwayMessage {
            shortcut_id: m.shortcut_id,
            recipients: BusinessRecipients::parse(m.recipients),
            schedule: BusinessAwayMessageSchedule::parse(&m.schedule),
            offline_only: m.offline_only,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BusinessAwayMessage, BusinessGreetingMessage};
    use crate::enums::BusinessAwayMessageSchedule;
    use crate::raw::{base, types};
    use crate::types::BusinessRecipients;

    fn recipients() -> base::BusinessRecipients {
        let r = types::BusinessRecipients {
            existing_chats: true,
            new_chats: false,
            contacts: true,
            non_contacts: false,
            exclude_selected: true,
            users: Some(vec![5, 6]),
        };
        base::BusinessRecipients::BusinessRecipients(r.into())
    }

    #[test]
    fn reads_greeting_and_away_messages() {
        let expected = BusinessRecipients {
            existing_chats: true,
            contacts: true,
            exclude_selected: true,
            users: vec![5, 6],
            ..Default::default()
        };
        let greeting = types::BusinessGreetingMessage { shortcut_id: 3, recipients: recipients(), no_activity_days: 7 };
        let greeting =
            BusinessGreetingMessage::parse(base::BusinessGreetingMessage::BusinessGreetingMessage(greeting.into()));
        assert_eq!(
            greeting,
            BusinessGreetingMessage { shortcut_id: 3, recipients: expected.clone(), no_activity_days: 7 }
        );

        let custom = types::BusinessAwayMessageScheduleCustom { start_date: 10, end_date: 20 };
        let away = types::BusinessAwayMessage {
            offline_only: true,
            shortcut_id: 4,
            schedule: base::BusinessAwayMessageSchedule::BusinessAwayMessageScheduleCustom(custom),
            recipients: recipients(),
        };
        let away = BusinessAwayMessage::parse(base::BusinessAwayMessage::BusinessAwayMessage(away.into()));
        let schedule = BusinessAwayMessageSchedule::Custom { start_date: 10, end_date: 20 };
        assert_eq!(away, BusinessAwayMessage { shortcut_id: 4, recipients: expected, schedule, offline_only: true });
    }
}
