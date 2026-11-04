use std::str;

use ::regex::{Captures, Regex};

use super::Filter;
use super::facts::Facts;
use super::pred::Pred;
use super::subjects::Subject;
use crate::types::{
    CallbackQuery, ChosenInlineResult, EphemeralMessage, GuestMessage, InlineQuery, Message, PreCheckoutQuery,
};

mod sealed {
    pub trait Search {
        /// What a `regex` filter reads off the update, which is what its facts carry.
        fn search_text(&self) -> Option<&str>;
    }
}

/// An update kind a regex can search: a message's text or caption, a callback's data, an inline query's text or a
/// payment's payload. Sealed.
pub trait Searchable: Subject + sealed::Search {}

macro_rules! searchable {
    ($($ty:ident => |$s:ident| $text:expr),* $(,)?) => {$(
        impl sealed::Search for $ty {
            fn search_text(&self) -> Option<&str> {
                let $s = self;
                Some($text)
            }
        }

        impl Searchable for $ty {}
    )*};
}

searchable! {
    Message => |s| s.raw.message.as_str(),
    GuestMessage => |s| s.message.raw.message.as_str(),
    EphemeralMessage => |s| s.raw.message.as_str(),
    CallbackQuery => |s| s.data.as_str(),
    InlineQuery => |s| s.query.as_str(),
    ChosenInlineResult => |s| s.query.as_str(),
    PreCheckoutQuery => |s| s.invoice_payload.as_str(),
}

/// What `regex` and `matches` search, from the facts: the text, or else the data when it's UTF-8. Nothing to search
/// when that's empty.
pub(super) fn seen<'a>(facts: &Facts<'a>) -> Option<&'a str> {
    let text = if facts.text.is_empty() { str::from_utf8(facts.data).ok()? } else { facts.text };
    (!text.is_empty()).then_some(text)
}

/// Updates whose text, caption, callback data or payload contains a match of `re`. An empty text never matches.
/// Read the groups of the match with [`matches`].
pub fn regex<T: Searchable>(re: Regex) -> Filter<T> {
    Filter(Pred::Regex(re))
}

/// Every match of `re` in the text `regex(re)` looks at, with its capture groups; none when there's no text.
pub fn matches<'a, T: Searchable>(re: &Regex, item: &'a T) -> Vec<Captures<'a>> {
    item.search_text().filter(|text| !text.is_empty()).map_or_else(Vec::new, |text| re.captures_iter(text).collect())
}

#[cfg(test)]
mod tests {
    use ::regex::Regex;

    use super::super::facts::PeerKinds;
    use super::super::subjects::private::Inspect;
    use super::super::{Facts, Filter};
    use super::{matches, regex, sealed::Search, seen};
    use crate::Client;
    use crate::client::Me;
    use crate::dispatcher::Entities;
    use crate::dispatcher::listeners::tests::query;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::ephemeral_message::tests::{channel, ephemeral};
    use crate::types::{ChosenInlineResult, EphemeralMessage, GuestMessage, InlineQuery, Message, PreCheckoutQuery};
    use crate::utils::get_peer;

    fn re(pattern: &str) -> Regex {
        Regex::new(pattern).unwrap()
    }

    fn bare(text: &str) -> types::Message {
        let mut raw = Message::bare(1, get_peer(7), 0);
        raw.message = text.into();
        raw
    }

    fn message(text: &str) -> Message {
        Message::parse(&Client::offline(), Box::new(bare(text)), &Entities::default())
    }

    fn hit<T: Inspect>(filter: &Filter<T>, item: &T) -> Option<bool> {
        let me = Me::default();
        filter.decide(&item.facts(&me))
    }

    fn inline_query(text: &str) -> InlineQuery {
        let raw = types::UpdateBotInlineQuery {
            query_id: 1,
            user_id: 1,
            query: text.into(),
            geo: None,
            peer_type: None,
            offset: String::new(),
        };
        InlineQuery::parse(&Client::offline(), &raw, &Entities::default())
    }

    fn chosen(text: &str) -> ChosenInlineResult {
        let raw =
            types::UpdateBotInlineSend { user_id: 1, query: text.into(), geo: None, id: "r".into(), msg_id: None };
        ChosenInlineResult::parse(&Client::offline(), &raw, &Entities::default())
    }

    fn checkout(payload: &[u8]) -> PreCheckoutQuery {
        let raw = types::UpdateBotPrecheckoutQuery {
            query_id: 1,
            user_id: 1,
            payload: payload.to_vec(),
            info: None,
            shipping_option_id: None,
            currency: "USD".into(),
            total_amount: 100,
        };
        PreCheckoutQuery::parse(&Client::offline(), &raw, &Entities::default())
    }

    #[test]
    fn matches_text_and_caption() {
        let f: Filter = regex(re(r"order (\d+)"));
        assert_eq!(hit(&f, &message("order 42")), Some(true));
        assert_eq!(hit(&f, &message("an order 7 please")), Some(true));
        assert_eq!(hit(&f, &message("order")), Some(false));
        // A caption is the message text too.
        let photo = base::MessageMedia::MessageMediaGeo(Box::new(types::MessageMediaGeo {
            geo: base::GeoPoint::GeoPointEmpty,
        }));
        let mut raw = bare("order 9");
        raw.media = Some(photo);
        let captioned = Message::parse(&Client::offline(), Box::new(raw), &Entities::default());
        assert_eq!(hit(&f, &captioned), Some(true));
        // Nothing to search is never a match, even for a pattern that matches the empty string.
        let anything: Filter = regex(re(""));
        assert_eq!(hit(&anything, &message("")), Some(false));
        assert_eq!(hit(&anything, &message("x")), Some(true));

        let ordered = message("order 42, item 7");
        let found = matches(&re(r"(\w+) (\d+)"), &ordered);
        let groups: Vec<(&str, &str)> = found.iter().map(|c| (c.get(1).unwrap().as_str(), &c[2])).collect();
        assert_eq!(groups, [("order", "42"), ("item", "7")]);
        assert!(matches(&re("x"), &message("")).is_empty());
    }

    #[test]
    fn callback_data_matches_only_as_utf8() {
        let me = Me::default();
        let f: Filter<crate::types::CallbackQuery> = regex(re("^buy:(\\d+)$"));
        let callback = |data: &[u8]| {
            let raw = types::UpdateBotCallbackQuery {
                query_id: 1,
                user_id: 8,
                peer: get_peer(7),
                msg_id: 2,
                chat_instance: 3,
                data: Some(data.to_vec()),
                game_short_name: None,
            };
            let facts = Facts::callback(&raw, PeerKinds::callback(&raw, &Entities::default()), &me);
            f.decide(&facts)
        };
        assert_eq!(callback(b"buy:12"), Some(true));
        assert_eq!(callback(b"sell:12"), Some(false));
        assert_eq!(callback(b"buy:1\xff"), Some(false));
        assert_eq!(callback(b""), Some(false));

        let tapped = query(&Client::offline(), 8, 7, 2);
        let tap: Filter<crate::types::CallbackQuery> = regex(re("^t(a)p$"));
        assert_eq!(hit(&tap, &tapped), Some(true));
        assert_eq!(&matches(&re("^t(a)p$"), &tapped)[0][1], "a");
    }

    #[test]
    fn inline_and_chosen_queries_match() {
        let f: Filter<InlineQuery> = regex(re("^cat"));
        assert_eq!(hit(&f, &inline_query("cats")), Some(true));
        assert_eq!(hit(&f, &inline_query("dog")), Some(false));
        assert_eq!(hit(&f, &inline_query("")), Some(false));
        let g: Filter<ChosenInlineResult> = regex(re("^cat"));
        assert_eq!(hit(&g, &chosen("cats")), Some(true));
        assert_eq!(hit(&g, &chosen("dog")), Some(false));
    }

    #[test]
    fn pre_checkout_payload_matches() {
        let f: Filter<PreCheckoutQuery> = regex(re(r"^inv-(\d+)$"));
        assert_eq!(hit(&f, &checkout(b"inv-77")), Some(true));
        assert_eq!(hit(&f, &checkout(b"order-77")), Some(false));
        assert_eq!(&matches(&re(r"^inv-(\d+)$"), &checkout(b"inv-77"))[0][1], "77");
    }

    #[test]
    fn regex_decides_on_raw_facts() {
        let me = Me::default();
        let f: Filter = regex(re("hi"));
        let raw = base::Message::Message(Box::new(bare("oh hi")));
        let facts = Facts::message(&raw, PeerKinds::message(&raw, &Entities::default()), &me);
        assert_eq!(f.decide(&facts), Some(true));
        assert_eq!(f.decide(&Facts::default()), Some(false));
    }

    #[test]
    fn search_text_equals_what_the_filter_saw() {
        let (client, me) = (Client::offline(), Me::default());
        let entities = Entities::default();
        let raw = types::UpdateBotGuestChatQuery {
            query_id: 1,
            message: base::Message::Message(Box::new(bare("guest text"))),
            reference_messages: None,
            qts: 0,
        };
        let guest = GuestMessage::parse(&client, raw, &entities).unwrap();
        let ephemeral = EphemeralMessage::parse(&client, ephemeral(false, Some(channel())), &entities);
        let tapped = query(&client, 8, 7, 2);
        fn same<T: Inspect + Search>(item: &T, me: &Me) {
            let facts = item.facts(me);
            assert_eq!(item.search_text().filter(|s| !s.is_empty()), seen(&facts));
        }
        same(&message("a b"), &me);
        same(&message(""), &me);
        same(&guest, &me);
        same(&ephemeral, &me);
        same(&tapped, &me);
        same(&inline_query("q"), &me);
        same(&inline_query(""), &me);
        same(&chosen("q"), &me);
        same(&checkout(b"p"), &me);
        assert_eq!(seen(&guest.facts(&me)), Some("guest text"));
        assert_eq!(seen(&ephemeral.facts(&me)), Some("secret"));
    }

    #[test]
    fn regex_composes_with_and_or_not() {
        let digits: Filter = regex(re(r"\d"));
        let word: Filter = regex(re("hi"));
        let both = digits.clone() & word.clone();
        let either = digits.clone() | word.clone();
        let neither = !(digits | word);
        for (text, expected) in
            [("hi 1", (true, true, false)), ("hi", (false, true, false)), ("x", (false, false, true))]
        {
            let m = message(text);
            let got = (hit(&both, &m), hit(&either, &m), hit(&neither, &m));
            assert_eq!(got, (Some(expected.0), Some(expected.1), Some(expected.2)), "{text:?}");
        }
    }
}
