use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultGame {
    parts: Parts,
}

impl InlineQueryResultGame {
    pub fn new(game_short_name: impl Into<String>) -> Self {
        InlineQueryResultGame { parts: Parts { stored: Some(Stored::Game(game_short_name.into())), ..Parts::new() } }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultGame: id, reply_markup);
