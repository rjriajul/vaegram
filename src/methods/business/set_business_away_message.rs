use crate::Client;
use crate::enums::BusinessAwayMessageSchedule;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::BusinessRecipients;

/// Request built by [`Client::set_business_away_message`]. Awaiting it returns whether Telegram accepted the message.
#[must_use = "a request does nothing until it is awaited"]
pub struct SetBusinessAwayMessage<'a> {
    client: &'a Client,
    shortcut_id: i32,
    recipients: &'a BusinessRecipients,
    schedule: BusinessAwayMessageSchedule,
    offline_only: bool,
}

impl Client {
    /// Sets the away message to the quick reply shortcut `shortcut_id`, sent to `recipients`.
    ///
    /// The message is active at all times unless [`schedule`](SetBusinessAwayMessage::schedule) narrows it.
    pub fn set_business_away_message<'a>(
        &'a self,
        shortcut_id: i32,
        recipients: &'a BusinessRecipients,
    ) -> SetBusinessAwayMessage<'a> {
        SetBusinessAwayMessage {
            client: self,
            shortcut_id,
            recipients,
            schedule: BusinessAwayMessageSchedule::Always,
            offline_only: false,
        }
    }
}

impl SetBusinessAwayMessage<'_> {
    /// Limits when the away message is active. Defaults to [`BusinessAwayMessageSchedule::Always`].
    pub fn schedule(mut self, schedule: BusinessAwayMessageSchedule) -> Self {
        self.schedule = schedule;
        self
    }

    /// Sends the away message only while the account is offline.
    pub fn offline_only(mut self) -> Self {
        self.offline_only = true;
        self
    }

    async fn send(self) -> Result<bool> {
        let message = types::InputBusinessAwayMessage {
            offline_only: self.offline_only,
            shortcut_id: self.shortcut_id,
            schedule: self.schedule.write(),
            recipients: self.recipients.write(self.client).await?,
        };
        let message = Some(base::InputBusinessAwayMessage::InputBusinessAwayMessage(Box::new(message)));
        self.client.invoke(&functions::account::UpdateBusinessAwayMessage { message }).await
    }
}

into_future!(SetBusinessAwayMessage, bool);
