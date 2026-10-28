use crate::raw::{base, types};
use crate::types::ReactionType;

#[derive(Debug, Clone, PartialEq)]
pub struct StoryArea {
    pub position: StoryAreaPosition,
    pub kind: StoryAreaType,
}

/// Where an area sits on the story, in percents of the media size; `x`/`y` are the center.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StoryAreaPosition {
    pub x_percentage: f64,
    pub y_percentage: f64,
    pub width_percentage: f64,
    pub height_percentage: f64,
    pub rotation_angle: f64,
    pub corner_radius_percentage: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StoryAreaType {
    Location {
        latitude: f64,
        longitude: f64,
        address: Option<LocationAddress>,
    },
    SuggestedReaction {
        reaction: ReactionType,
        is_dark: bool,
        is_flipped: bool,
    },
    Link {
        url: String,
    },
    /// `background_color` is ARGB.
    Weather {
        temperature: f64,
        emoji: String,
        background_color: i32,
    },
    /// `name` is the unique gift's slug.
    UniqueGift {
        name: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocationAddress {
    /// Two-letter ISO 3166-1 alpha-2 code.
    pub country_code: String,
    pub state: Option<String>,
    pub city: Option<String>,
    pub street: Option<String>,
}

impl StoryArea {
    pub(crate) fn write(&self) -> base::MediaArea {
        let p = &self.position;
        let coordinates = base::MediaAreaCoordinates::MediaAreaCoordinates(Box::new(types::MediaAreaCoordinates {
            x: p.x_percentage,
            y: p.y_percentage,
            w: p.width_percentage,
            h: p.height_percentage,
            rotation: p.rotation_angle,
            radius: Some(p.corner_radius_percentage),
        }));
        match &self.kind {
            StoryAreaType::Location { latitude, longitude, address } => {
                let geo = types::GeoPoint { long: *longitude, lat: *latitude, access_hash: 0, accuracy_radius: None };
                let address = address.as_ref().map(|a| {
                    base::GeoPointAddress::GeoPointAddress(Box::new(types::GeoPointAddress {
                        country_iso2: a.country_code.clone(),
                        state: a.state.clone(),
                        city: a.city.clone(),
                        street: a.street.clone(),
                    }))
                });
                let geo = base::GeoPoint::GeoPoint(Box::new(geo));
                base::MediaArea::MediaAreaGeoPoint(Box::new(types::MediaAreaGeoPoint { coordinates, geo, address }))
            }
            StoryAreaType::SuggestedReaction { reaction, is_dark, is_flipped } => {
                base::MediaArea::MediaAreaSuggestedReaction(Box::new(types::MediaAreaSuggestedReaction {
                    dark: *is_dark,
                    flipped: *is_flipped,
                    coordinates,
                    reaction: reaction.write(),
                }))
            }
            StoryAreaType::Link { url } => {
                base::MediaArea::MediaAreaUrl(Box::new(types::MediaAreaUrl { coordinates, url: url.clone() }))
            }
            StoryAreaType::Weather { temperature, emoji, background_color } => {
                base::MediaArea::MediaAreaWeather(Box::new(types::MediaAreaWeather {
                    coordinates,
                    emoji: emoji.clone(),
                    temperature_c: *temperature,
                    color: *background_color,
                }))
            }
            StoryAreaType::UniqueGift { name } => {
                let gift = types::MediaAreaStarGift { coordinates, slug: name.clone() };
                base::MediaArea::MediaAreaStarGift(Box::new(gift))
            }
        }
    }
}
