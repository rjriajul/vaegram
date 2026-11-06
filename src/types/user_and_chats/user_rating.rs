use crate::raw::base;

/// A user's rating, earned with Stars spent on gifts and paid content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserRating {
    /// Current level.
    pub level: i32,
    /// The user is at the top level, so `next_level_rating` is `None`.
    pub is_maximum_level_reached: bool,
    /// Rating the user has now.
    pub rating: i64,
    /// The rating where the current level starts.
    pub current_level_rating: i64,
    /// The rating where the next level starts. `None` at the top level.
    pub next_level_rating: Option<i64>,
}

impl UserRating {
    pub(crate) fn parse(raw: &base::StarsRating) -> UserRating {
        let base::StarsRating::StarsRating(r) = raw;
        UserRating {
            level: r.level,
            is_maximum_level_reached: r.next_level_stars.is_none(),
            rating: r.stars,
            current_level_rating: r.current_level_stars,
            next_level_rating: r.next_level_stars,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UserRating;
    use crate::raw::{base, types};

    #[test]
    fn the_top_level_has_no_next_level() {
        let parse = |next_level_stars| {
            let raw = types::StarsRating { level: 4, current_level_stars: 100, stars: 150, next_level_stars };
            UserRating::parse(&base::StarsRating::StarsRating(raw.into()))
        };
        let climbing = parse(Some(200));
        assert_eq!((climbing.level, climbing.rating, climbing.current_level_rating), (4, 150, 100));
        assert_eq!((climbing.next_level_rating, climbing.is_maximum_level_reached), (Some(200), false));
        assert!(parse(None).is_maximum_level_reached);
    }
}
