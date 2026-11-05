use super::inline_query_result::{Parts, Stored, setters};

/// A game registered with @BotFather.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultGame {
    parts: Parts,
}

impl InlineQueryResultGame {
    /// Creates a result for the game with the given short name.
    pub fn new(game_short_name: impl Into<String>) -> Self {
        InlineQueryResultGame { parts: Parts { stored: Some(Stored::Game(game_short_name.into())), ..Parts::new() } }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultGame: id, reply_markup);
