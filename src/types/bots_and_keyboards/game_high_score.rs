use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;

/// One entry of a game's high score table.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GameHighScore {
    /// Place in the table.
    pub position: i32,
    /// The player.
    pub user: User,
    /// The player's score.
    pub score: i32,
}

impl GameHighScore {
    pub(crate) fn parse(raw: &base::HighScore, entities: &Entities) -> GameHighScore {
        let base::HighScore::HighScore(s) = raw;
        GameHighScore { position: s.pos, user: User::parse(s.user_id, entities), score: s.score }
    }
}
