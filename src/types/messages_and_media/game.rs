use crate::raw::base;
use crate::types::{Animation, Media, Photo};

/// A game attached to a message.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Game {
    /// Telegram's id of the game.
    pub id: i64,
    /// Short name that identifies the game to its bot.
    pub short_name: String,
    /// Title of the game.
    pub title: String,
    /// Description of the game.
    pub description: String,
    /// Photo of the game. `None` if the game has no photo.
    pub photo: Option<Photo>,
    /// Animation of the game. `None` if the game has no document or it isn't an animation.
    pub animation: Option<Animation>,
}

impl Game {
    pub(crate) fn parse(raw: &base::Game) -> Game {
        let base::Game::Game(g) = raw;
        Game {
            id: g.id,
            short_name: g.short_name.clone(),
            title: g.title.clone(),
            description: g.description.clone(),
            photo: Photo::parse(&g.photo),
            animation: match g.document.as_ref().and_then(Media::from_document) {
                Some(Media::Animation(a)) => Some(a),
                _ => None,
            },
        }
    }
}
