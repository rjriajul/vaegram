use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameHighScore {
    pub position: i32,
    pub user: User,
    pub score: i32,
}

impl GameHighScore {
    pub(crate) fn parse(raw: &base::HighScore, entities: &Entities) -> GameHighScore {
        let base::HighScore::HighScore(s) = raw;
        GameHighScore { position: s.pos, user: User::parse(s.user_id, entities), score: s.score }
    }
}
