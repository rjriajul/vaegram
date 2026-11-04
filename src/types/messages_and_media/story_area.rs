use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::input_venue;
use crate::raw::{base, types};
use crate::types::{Chat, PeerRef, ReactionType, Venue};
use crate::utils::{get_channel_id, get_peer};

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
#[non_exhaustive]
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
    Venue(Venue),
    /// A link to a post of a channel. Writing it needs the channel to be known to the client.
    ChannelPost {
        chat: Box<Chat>,
        message_id: i32,
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

impl StoryAreaPosition {
    fn parse(coordinates: &base::MediaAreaCoordinates) -> StoryAreaPosition {
        let base::MediaAreaCoordinates::MediaAreaCoordinates(c) = coordinates;
        StoryAreaPosition {
            x_percentage: c.x,
            y_percentage: c.y,
            width_percentage: c.w,
            height_percentage: c.h,
            rotation_angle: c.rotation,
            corner_radius_percentage: c.radius.unwrap_or_default(),
        }
    }

    fn write(&self) -> base::MediaAreaCoordinates {
        base::MediaAreaCoordinates::MediaAreaCoordinates(Box::new(types::MediaAreaCoordinates {
            x: self.x_percentage,
            y: self.y_percentage,
            w: self.width_percentage,
            h: self.height_percentage,
            rotation: self.rotation_angle,
            radius: Some(self.corner_radius_percentage),
        }))
    }
}

impl StoryArea {
    /// `None` for the `input*` areas, which only exist in requests, and for areas without a usable place.
    pub(crate) fn parse(area: &base::MediaArea, entities: &Entities) -> Option<StoryArea> {
        use base::MediaArea as A;
        let (coordinates, kind) = match area {
            A::MediaAreaVenue(a) => {
                let venue = Venue::from_provider(
                    &a.geo,
                    a.title.clone(),
                    a.address.clone(),
                    &a.provider,
                    &a.venue_id,
                    &a.venue_type,
                )?;
                (&a.coordinates, StoryAreaType::Venue(venue))
            }
            A::MediaAreaGeoPoint(a) => {
                let base::GeoPoint::GeoPoint(geo) = &a.geo else { return None };
                let address = a.address.as_ref().map(|base::GeoPointAddress::GeoPointAddress(a)| LocationAddress {
                    country_code: a.country_iso2.clone(),
                    state: a.state.clone(),
                    city: a.city.clone(),
                    street: a.street.clone(),
                });
                (&a.coordinates, StoryAreaType::Location { latitude: geo.lat, longitude: geo.long, address })
            }
            A::MediaAreaSuggestedReaction(a) => {
                let reaction = ReactionType::parse(&a.reaction)?;
                (&a.coordinates, StoryAreaType::SuggestedReaction { reaction, is_dark: a.dark, is_flipped: a.flipped })
            }
            A::MediaAreaChannelPost(a) => {
                let chat = Box::new(Chat::parse(&get_peer(get_channel_id(a.channel_id)), entities, false));
                (&a.coordinates, StoryAreaType::ChannelPost { chat, message_id: a.msg_id })
            }
            A::MediaAreaUrl(a) => (&a.coordinates, StoryAreaType::Link { url: a.url.clone() }),
            A::MediaAreaWeather(a) => (
                &a.coordinates,
                StoryAreaType::Weather {
                    temperature: a.temperature_c,
                    emoji: a.emoji.clone(),
                    background_color: a.color,
                },
            ),
            A::MediaAreaStarGift(a) => (&a.coordinates, StoryAreaType::UniqueGift { name: a.slug.clone() }),
            A::InputMediaAreaVenue(_) | A::InputMediaAreaChannelPost(_) => return None,
        };
        Some(StoryArea { position: StoryAreaPosition::parse(coordinates), kind })
    }

    pub(crate) fn parse_all(areas: &[base::MediaArea], entities: &Entities) -> Vec<StoryArea> {
        areas.iter().filter_map(|a| StoryArea::parse(a, entities)).collect()
    }

    pub(crate) async fn write(&self, client: &Client) -> Result<base::MediaArea> {
        let coordinates = self.position.write();
        Ok(match &self.kind {
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
            StoryAreaType::Venue(venue) => {
                let input = input_venue(venue.clone());
                let location = &venue.location;
                let geo = types::GeoPoint {
                    long: location.longitude,
                    lat: location.latitude,
                    access_hash: 0,
                    accuracy_radius: location.horizontal_accuracy,
                };
                base::MediaArea::MediaAreaVenue(Box::new(types::MediaAreaVenue {
                    coordinates,
                    geo: base::GeoPoint::GeoPoint(Box::new(geo)),
                    title: input.title,
                    address: input.address,
                    provider: input.provider,
                    venue_id: input.venue_id,
                    venue_type: input.venue_type,
                }))
            }
            StoryAreaType::ChannelPost { chat, message_id } => {
                let channel = client.resolve_channel(&PeerRef::from(chat.id)).await?;
                base::MediaArea::InputMediaAreaChannelPost(Box::new(types::InputMediaAreaChannelPost {
                    coordinates,
                    channel,
                    msg_id: *message_id,
                }))
            }
        })
    }

    /// Writes every area, keeping their order.
    pub(crate) async fn write_all(areas: &[StoryArea], client: &Client) -> Result<Vec<base::MediaArea>> {
        let mut written = Vec::with_capacity(areas.len());
        for area in areas {
            written.push(area.write(client).await?);
        }
        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::{LocationAddress, StoryArea, StoryAreaType};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::{ReactionType, Venue};
    use crate::utils::get_channel_id;

    fn coords() -> base::MediaAreaCoordinates {
        base::MediaAreaCoordinates::MediaAreaCoordinates(Box::new(types::MediaAreaCoordinates {
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            rotation: 5.0,
            radius: Some(2.5),
        }))
    }

    fn geo(lat: f64, long: f64) -> base::GeoPoint {
        base::GeoPoint::GeoPoint(Box::new(types::GeoPoint { long, lat, access_hash: 0, accuracy_radius: None }))
    }

    /// One raw area per constructor that can come back from Telegram, paired with the kind it must parse to.
    fn readable() -> Vec<(base::MediaArea, StoryAreaType)> {
        let c = coords;
        let address = types::GeoPointAddress {
            country_iso2: "DE".into(),
            state: None,
            city: Some("Berlin".into()),
            street: None,
        };
        vec![
            (
                base::MediaArea::MediaAreaVenue(Box::new(types::MediaAreaVenue {
                    coordinates: c(),
                    geo: geo(1.0, 2.0),
                    title: "Cafe".into(),
                    address: "Main St".into(),
                    provider: "foursquare".into(),
                    venue_id: "4b5".into(),
                    venue_type: "food/cafe".into(),
                })),
                StoryAreaType::Venue({
                    let mut v = Venue::new(1.0, 2.0, "Cafe", "Main St");
                    (v.foursquare_id, v.foursquare_type) = (Some("4b5".into()), Some("food/cafe".into()));
                    v
                }),
            ),
            (
                base::MediaArea::MediaAreaVenue(Box::new(types::MediaAreaVenue {
                    coordinates: c(),
                    geo: geo(5.0, 6.0),
                    title: "Park".into(),
                    address: "Elm St".into(),
                    provider: "gplaces".into(),
                    venue_id: "ChIJ".into(),
                    venue_type: "park".into(),
                })),
                StoryAreaType::Venue({
                    let mut v = Venue::new(5.0, 6.0, "Park", "Elm St");
                    (v.google_place_id, v.google_place_type) = (Some("ChIJ".into()), Some("park".into()));
                    v
                }),
            ),
            (
                base::MediaArea::MediaAreaGeoPoint(Box::new(types::MediaAreaGeoPoint {
                    coordinates: c(),
                    geo: geo(3.0, 4.0),
                    address: Some(base::GeoPointAddress::GeoPointAddress(Box::new(address))),
                })),
                StoryAreaType::Location {
                    latitude: 3.0,
                    longitude: 4.0,
                    address: Some(LocationAddress {
                        country_code: "DE".into(),
                        city: Some("Berlin".into()),
                        ..Default::default()
                    }),
                },
            ),
            (
                base::MediaArea::MediaAreaSuggestedReaction(Box::new(types::MediaAreaSuggestedReaction {
                    dark: true,
                    flipped: false,
                    coordinates: c(),
                    reaction: base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "🔥".into() })),
                })),
                StoryAreaType::SuggestedReaction {
                    reaction: ReactionType::Emoji("🔥".into()),
                    is_dark: true,
                    is_flipped: false,
                },
            ),
            (
                base::MediaArea::MediaAreaUrl(Box::new(types::MediaAreaUrl {
                    coordinates: c(),
                    url: "https://example.com".into(),
                })),
                StoryAreaType::Link { url: "https://example.com".into() },
            ),
            (
                base::MediaArea::MediaAreaWeather(Box::new(types::MediaAreaWeather {
                    coordinates: c(),
                    emoji: "☀".into(),
                    temperature_c: 21.5,
                    color: -1,
                })),
                StoryAreaType::Weather { temperature: 21.5, emoji: "☀".into(), background_color: -1 },
            ),
            (
                base::MediaArea::MediaAreaStarGift(Box::new(types::MediaAreaStarGift {
                    coordinates: c(),
                    slug: "Plush-1".into(),
                })),
                StoryAreaType::UniqueGift { name: "Plush-1".into() },
            ),
        ]
    }

    #[test]
    fn every_raw_area_parses() {
        let entities = Entities::default();
        for (raw, kind) in readable() {
            let area = StoryArea::parse(&raw, &entities).unwrap();
            assert_eq!(area.kind, kind);
            assert_eq!(
                (area.position.x_percentage, area.position.height_percentage, area.position.corner_radius_percentage),
                (10.0, 40.0, 2.5)
            );
        }

        let post = types::MediaAreaChannelPost { coordinates: coords(), channel_id: 9, msg_id: 7 };
        let area = StoryArea::parse(&base::MediaArea::MediaAreaChannelPost(Box::new(post)), &entities).unwrap();
        let StoryAreaType::ChannelPost { chat, message_id } = area.kind else { panic!("not a channel post") };
        assert_eq!((chat.id, message_id), (get_channel_id(9), 7));

        // A venue without a point has nowhere to sit.
        let nowhere = types::MediaAreaVenue {
            coordinates: coords(),
            geo: base::GeoPoint::GeoPointEmpty,
            title: String::new(),
            address: String::new(),
            provider: String::new(),
            venue_id: String::new(),
            venue_type: String::new(),
        };
        assert!(StoryArea::parse(&base::MediaArea::MediaAreaVenue(Box::new(nowhere)), &entities).is_none());

        let input = types::InputMediaAreaVenue { coordinates: coords(), query_id: 1, result_id: "r".into() };
        assert!(StoryArea::parse(&base::MediaArea::InputMediaAreaVenue(Box::new(input)), &entities).is_none());
        let channel = types::InputChannel { channel_id: 9, access_hash: 1 };
        let input = types::InputMediaAreaChannelPost {
            coordinates: coords(),
            channel: base::InputChannel::InputChannel(Box::new(channel)),
            msg_id: 7,
        };
        assert!(StoryArea::parse(&base::MediaArea::InputMediaAreaChannelPost(Box::new(input)), &entities).is_none());
    }

    #[tokio::test]
    async fn areas_write_back_as_sent() {
        let client = Client::offline();
        let entities = Entities::default();
        for (raw, _) in readable() {
            let area = StoryArea::parse(&raw, &entities).unwrap();
            assert_eq!(area.write(&client).await.unwrap(), raw);
        }

        let channel = types::InputChannel { channel_id: 9, access_hash: 1 };
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(peer)));
        let post = types::MediaAreaChannelPost { coordinates: coords(), channel_id: 9, msg_id: 7 };
        let area = StoryArea::parse(&base::MediaArea::MediaAreaChannelPost(Box::new(post)), &entities).unwrap();
        let expected = types::InputMediaAreaChannelPost {
            coordinates: coords(),
            channel: base::InputChannel::InputChannel(Box::new(channel)),
            msg_id: 7,
        };
        assert_eq!(area.write(&client).await.unwrap(), base::MediaArea::InputMediaAreaChannelPost(Box::new(expected)));
    }
}
