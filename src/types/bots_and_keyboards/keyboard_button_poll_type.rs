use crate::enums::PollType;
use crate::raw::{base, types};

/// A button that asks the user to make a poll. Without a type, any poll is accepted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyboardButtonPollType {
    /// The kind of poll to accept. `None` accepts any.
    pub poll_type: Option<PollType>,
}

impl KeyboardButtonPollType {
    /// Accepts only polls of this type.
    pub fn new(poll_type: PollType) -> Self {
        KeyboardButtonPollType { poll_type: Some(poll_type) }
    }

    pub(crate) fn write(&self) -> base::ButtonType {
        let quiz = self.poll_type.map(|t| t == PollType::Quiz);
        base::ButtonType::ButtonTypeRequestPoll(types::ButtonTypeRequestPoll { quiz })
    }

    pub(crate) fn parse(raw: &types::ButtonTypeRequestPoll) -> KeyboardButtonPollType {
        let poll_type = raw.quiz.map(|quiz| if quiz { PollType::Quiz } else { PollType::Regular });
        KeyboardButtonPollType { poll_type }
    }
}
