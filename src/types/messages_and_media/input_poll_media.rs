use crate::Client;
use crate::errors::Result;
use crate::methods::{geo_point, input_venue};
use crate::raw::{base, types};
use crate::session::transfer::try_join_all;
use crate::types::{InputMedia, Location, PeerRef, Venue};

pub struct InputPollMedia<'a>(Kind<'a>);

enum Kind<'a> {
    File(Box<InputMedia<'a>>),
    Ready(base::InputMedia),
}

impl InputPollMedia<'_> {
    /// For accuracy, pass a [`Location`] instead.
    pub fn location(latitude: f64, longitude: f64) -> Self {
        Location::new(latitude, longitude).into()
    }

    /// For a Foursquare or Google Places id, pass a [`Venue`] instead.
    pub fn venue(latitude: f64, longitude: f64, title: impl Into<String>, address: impl Into<String>) -> Self {
        Venue::new(latitude, longitude, title, address).into()
    }

    pub fn link(url: impl Into<String>) -> Self {
        InputPollMedia(Kind::Ready(base::InputMedia::InputMediaWebPage(Box::new(types::InputMediaWebPage {
            force_large_media: false,
            force_small_media: false,
            optional: false,
            url: url.into(),
        }))))
    }

    pub(crate) async fn upload(
        self,
        client: &Client,
        peer: &base::InputPeer,
        business: Option<&str>,
    ) -> Result<base::InputMedia> {
        match self.0 {
            Kind::File(media) => {
                client.upload_media(peer, client.input_media(peer, &media, None, business).await?, business).await
            }
            Kind::Ready(media) => Ok(media),
        }
    }
}

impl<'a> From<InputMedia<'a>> for InputPollMedia<'a> {
    fn from(media: InputMedia<'a>) -> Self {
        InputPollMedia(Kind::File(Box::new(media)))
    }
}

impl From<Location> for InputPollMedia<'_> {
    fn from(location: Location) -> Self {
        let geo_point = geo_point(location.latitude, location.longitude, location.horizontal_accuracy);
        InputPollMedia(Kind::Ready(base::InputMedia::InputMediaGeoPoint(Box::new(types::InputMediaGeoPoint {
            geo_point,
        }))))
    }
}

impl From<Venue> for InputPollMedia<'_> {
    fn from(venue: Venue) -> Self {
        InputPollMedia(Kind::Ready(base::InputMedia::InputMediaVenue(Box::new(input_venue(venue)))))
    }
}

#[derive(Default)]
pub(crate) struct PollMedia<'a> {
    pub(crate) description: Option<InputPollMedia<'a>>,
    pub(crate) explanation: Option<InputPollMedia<'a>>,
    pub(crate) options: Vec<Option<InputPollMedia<'a>>>,
}

impl PollMedia<'_> {
    pub(crate) async fn attach(
        self,
        client: &Client,
        chat_id: &PeerRef,
        business: Option<&str>,
        media: &mut base::InputMedia,
    ) -> Result<()> {
        let base::InputMedia::InputMediaPoll(poll) = media else { return Ok(()) };
        let mut all = self.description.iter().chain(&self.explanation).chain(self.options.iter().flatten());
        // Places and links need no chat; only files are stored in it first, like album items.
        let peer = match all.any(|m| matches!(m.0, Kind::File(_))) {
            true => client.resolve(chat_id).await?.1,
            false => base::InputPeer::InputPeerEmpty,
        };
        let peer = &peer;
        let items = [self.description, self.explanation].into_iter().chain(self.options);
        let mut uploaded = try_join_all(items.map(|item| async move {
            match item {
                Some(item) => item.upload(client, peer, business).await.map(Some),
                None => Ok(None),
            }
        }))
        .await?
        .into_iter();
        poll.attached_media = uploaded.next().flatten();
        poll.solution_media = uploaded.next().flatten();
        let base::Poll::Poll(raw) = &mut poll.poll;
        for (answer, media) in raw.answers.iter_mut().zip(uploaded) {
            if let (base::PollAnswer::InputPollAnswer(answer), Some(media)) = (answer, media) {
                answer.media = Some(media);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{InputPollMedia, Kind};
    use crate::raw::base;
    use crate::types::{InputMedia, Location, Venue};

    fn ready(media: InputPollMedia) -> base::InputMedia {
        match media.0 {
            Kind::Ready(media) => media,
            Kind::File(_) => panic!("files need an upload"),
        }
    }

    #[test]
    fn maps_places_and_links() {
        let location = Location { horizontal_accuracy: Some(30), ..Location::new(1.5, 2.5) };
        let base::InputMedia::InputMediaGeoPoint(geo) = ready(location.into()) else { panic!("not a location") };
        let base::InputGeoPoint::InputGeoPoint(point) = &geo.geo_point else { panic!("empty point") };
        assert_eq!((point.lat, point.long, point.accuracy_radius), (1.5, 2.5, Some(30)));

        let venue = |media: InputPollMedia| match ready(media) {
            base::InputMedia::InputMediaVenue(v) => (v.title, v.provider, v.venue_id, v.venue_type),
            _ => panic!("not a venue"),
        };
        let plain = venue(InputPollMedia::venue(1.0, 2.0, "Cafe", "Main St"));
        assert_eq!(plain, ("Cafe".into(), String::new(), String::new(), String::new()));
        let google = Venue {
            google_place_id: Some("ChIJ".into()),
            google_place_type: Some("cafe".into()),
            ..Venue::new(1.0, 2.0, "Cafe", "Main St")
        };
        assert_eq!(venue(google.into()), ("Cafe".into(), "gplaces".into(), "ChIJ".into(), "cafe".into()));

        let base::InputMedia::InputMediaWebPage(page) = ready(InputPollMedia::link("https://telegram.org")) else {
            panic!("not a link")
        };
        assert_eq!((page.url.as_str(), page.optional), ("https://telegram.org", false));
        assert!(matches!(InputPollMedia::from(InputMedia::photo("a.jpg")).0, Kind::File(_)));
    }
}
