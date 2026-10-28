use crate::raw::base;
use crate::types::{Animation, Media, Photo};

#[derive(Debug, Clone)]
pub struct Game {
    pub id: i64,
    pub short_name: String,
    pub title: String,
    pub description: String,
    pub photo: Option<Photo>,
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
