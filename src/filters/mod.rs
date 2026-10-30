mod facts;
mod pred;
mod subjects;

use std::ops::{BitAnd, BitOr, Not};
use std::sync::Arc;

pub(crate) use facts::{Facts, PeerKinds};
use pred::Pred;
pub(crate) use subjects::private::Inspect;
pub use subjects::{HasChat, HasData, HasMedia, HasSender, HasText, Subject};

use crate::enums::{ChatType, MediaKind, ServiceKind};
use crate::types::Message;

pub struct Filter<T = Message>(Pred<T>);

impl<T> Clone for Filter<T> {
    fn clone(&self) -> Self {
        Filter(self.0.clone())
    }
}

impl<T: Subject + Inspect> Filter<T> {
    pub fn check(&self, item: &T) -> bool {
        let me = item.client().0.me.lock().unwrap().clone();
        self.eval(&item.facts(&me), item)
    }
}

impl<T> Filter<T> {
    pub(crate) fn eval(&self, facts: &Facts<'_>, item: &T) -> bool {
        self.0.eval(facts, Some(item))
    }

    pub(crate) fn decide(&self, facts: &Facts<'_>) -> Option<bool> {
        self.0.decide(facts)
    }
}

impl<T> BitAnd for Filter<T> {
    type Output = Filter<T>;
    fn bitand(self, other: Filter<T>) -> Filter<T> {
        Filter(self.0.and(other.0))
    }
}

impl<T> BitOr for Filter<T> {
    type Output = Filter<T>;
    fn bitor(self, other: Filter<T>) -> Filter<T> {
        Filter(self.0.or(other.0))
    }
}

impl<T> Not for Filter<T> {
    type Output = Filter<T>;
    fn not(self) -> Filter<T> {
        Filter(match self.0 {
            Pred::Not(p) => *p,
            p => Pred::Not(Box::new(p)),
        })
    }
}

/// Chat or user ids for `chat` and `user`: one id, an array, a slice or a `Vec`.
pub struct Ids(Box<[i64]>);

impl Ids {
    fn new(mut ids: Vec<i64>) -> Ids {
        ids.sort_unstable();
        ids.dedup();
        Ids(ids.into())
    }
}

impl From<i64> for Ids {
    fn from(id: i64) -> Ids {
        Ids(Box::new([id]))
    }
}

impl<const N: usize> From<[i64; N]> for Ids {
    fn from(ids: [i64; N]) -> Ids {
        Ids::new(ids.into())
    }
}

impl From<&[i64]> for Ids {
    fn from(ids: &[i64]) -> Ids {
        Ids::new(ids.into())
    }
}

impl From<Vec<i64>> for Ids {
    fn from(ids: Vec<i64>) -> Ids {
        Ids::new(ids)
    }
}

/// Command names for `command`: `"start"` or `["ban", "kick"]`. Matched without case.
pub trait CommandNames {
    fn names(self) -> Box<[Box<str>]>;
}

impl CommandNames for &str {
    fn names(self) -> Box<[Box<str>]> {
        Box::new([self.into()])
    }
}

impl CommandNames for String {
    fn names(self) -> Box<[Box<str>]> {
        Box::new([self.into()])
    }
}

impl<const N: usize> CommandNames for [&str; N] {
    fn names(self) -> Box<[Box<str>]> {
        self.iter().map(|&n| n.into()).collect()
    }
}

impl CommandNames for &[&str] {
    fn names(self) -> Box<[Box<str>]> {
        self.iter().map(|&n| n.into()).collect()
    }
}

impl CommandNames for Vec<String> {
    fn names(self) -> Box<[Box<str>]> {
        self.into_iter().map(Into::into).collect()
    }
}

pub fn all<T: Subject>() -> Filter<T> {
    Filter(Pred::All)
}

/// A filter from a closure. It needs the parsed update, so put cheap filters before it with `&`: a handler whose
/// filter starts with a closure parses every update of its kind.
pub fn create<T: Subject>(f: impl Fn(&T) -> bool + Send + Sync + 'static) -> Filter<T> {
    Filter(Pred::Custom(Arc::new(f)))
}

fn flags<T>(set: u32) -> Filter<T> {
    Filter(Pred::Flags { set, clear: 0 })
}

fn chat_kinds<T>(kinds: &[ChatType]) -> Filter<T> {
    Filter(Pred::ChatKinds(kinds.iter().fold(0, |mask, &k| mask | 1 << k as u8)))
}

/// Text without media. Link previews don't count as media.
pub fn text<T: HasText>() -> Filter<T> {
    flags(facts::TEXT)
}

pub fn caption<T: HasMedia>() -> Filter<T> {
    flags(facts::CAPTION)
}

/// `/name`, or `/name@us`; never `/name@another_bot`. Read it in the handler with `Message::command`.
pub fn command<T: HasText>(names: impl CommandNames) -> Filter<T> {
    command_with("/", names)
}

pub fn command_with<T: HasText>(prefixes: &str, names: impl CommandNames) -> Filter<T> {
    assert!(!prefixes.is_empty(), "command prefixes can't be empty; use \"/\" for bot commands");
    Filter(Pred::Commands { prefixes: prefixes.into(), names: names.names() })
}

pub fn text_starts_with<T: HasText>(prefix: &str) -> Filter<T> {
    Filter(Pred::TextPrefix(prefix.into()))
}

pub fn media<T: HasMedia>() -> Filter<T> {
    Filter(Pred::Media(!(1 << MediaKind::WebPage as u8)))
}

macro_rules! media_filters {
    ($($name:ident => $kind:ident),* $(,)?) => {$(
        pub fn $name<T: HasMedia>() -> Filter<T> {
            Filter(Pred::Media(1 << MediaKind::$kind as u8))
        }
    )*};
}

media_filters! {
    photo => Photo, video => Video, animation => Animation, audio => Audio, voice => Voice, video_note => VideoNote,
    document => Document, sticker => Sticker, live_photo => LivePhoto, contact => Contact, location => Location,
    venue => Venue, dice => Dice, poll => Poll, game => Game, invoice => Invoice, checklist => Checklist,
    story => Story, giveaway => Giveaway, paid_media => PaidMedia, web_page => WebPage,
}

macro_rules! flag_filters {
    ($($(#[$doc:meta])* $name:ident: $cap:ident => $flag:ident),* $(,)?) => {$(
        $(#[$doc])*
        pub fn $name<T: $cap>() -> Filter<T> {
            flags(facts::$flag)
        }
    )*};
}

flag_filters! {
    reply: HasMedia => REPLY,
    forwarded: HasMedia => FORWARDED,
    via_bot: HasMedia => VIA_BOT,
    mentioned: HasMedia => MENTIONED,
    scheduled: HasMedia => SCHEDULED,
    protected: HasMedia => PROTECTED,
    media_group: HasMedia => MEDIA_GROUP,
    service: HasMedia => SERVICE,
    outgoing: HasChat => OUTGOING,
    me: HasSender => SENDER_ME,
    bot: HasSender => SENDER_BOT,
}

pub fn incoming<T: HasChat>() -> Filter<T> {
    Filter(Pred::Flags { set: 0, clear: facts::OUTGOING })
}

pub fn topic<T: HasMedia>(id: i32) -> Filter<T> {
    flags(facts::TOPIC) & Filter(Pred::Thread(id))
}

pub fn service_kind<T: HasMedia>(kinds: impl IntoIterator<Item = ServiceKind>) -> Filter<T> {
    Filter(Pred::Service(kinds.into_iter().fold(0, |mask, k| mask | 1 << k as u8)))
}

pub fn private<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Private, ChatType::Bot])
}

pub fn group<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Group, ChatType::Supergroup])
}

pub fn supergroup<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Supergroup])
}

pub fn channel<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Channel])
}

pub fn chat<T: HasChat>(ids: impl Into<Ids>) -> Filter<T> {
    Filter(Pred::Chats(ids.into().0))
}

pub fn user<T: HasSender>(ids: impl Into<Ids>) -> Filter<T> {
    Filter(Pred::Users(ids.into().0))
}

pub fn data<T: HasData>(value: impl AsRef<[u8]>) -> Filter<T> {
    Filter(Pred::Data(value.as_ref().into()))
}

pub fn data_prefix<T: HasData>(prefix: impl AsRef<[u8]>) -> Filter<T> {
    Filter(Pred::DataPrefix(prefix.as_ref().into()))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::facts::*;
    use super::*;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::CallbackQuery;
    use crate::utils::get_peer;

    fn message(client: &Client, chat_id: i64, text: &str, media: Option<base::MessageMedia>) -> Message {
        let mut raw = Message::bare(1, get_peer(chat_id), 0);
        (raw.message, raw.media) = (text.into(), media);
        Message::parse(client, Box::new(raw), &Entities::default())
    }

    fn hit<T>(filter: &Filter<T>, facts: Facts<'_>) -> bool {
        filter.decide(&facts).unwrap()
    }

    #[test]
    fn every_leaf_matches_and_rejects() {
        let f = Facts::default();
        let on = |flags| Facts { flags, ..f };
        let says = |text| Facts { text, me: Some("Bot"), ..f };
        let holds = |m| Facts { media: Some(m), ..f };
        let inside = |c| Facts { chat: Some(c), ..f };
        let cases: Vec<(Filter, Facts, Facts)> = vec![
            (text(), on(TEXT), on(CAPTION)),
            (caption(), on(CAPTION), on(TEXT)),
            (command("start"), says("/START@bot x"), says("/start@other")),
            (command(["ban", "kick"]), says("/kick"), says("/kicked")),
            (command_with("!.", "ban"), says(".ban 1"), says("/ban")),
            (text_starts_with("hi"), says("hi there"), says("oh hi")),
            (media(), holds(MediaKind::Photo), holds(MediaKind::WebPage)),
            (photo(), holds(MediaKind::Photo), holds(MediaKind::Video)),
            (sticker(), holds(MediaKind::Sticker), holds(MediaKind::Document)),
            (video_note(), holds(MediaKind::VideoNote), f),
            (reply(), on(REPLY), f),
            (forwarded(), on(FORWARDED), f),
            (via_bot(), on(VIA_BOT), f),
            (mentioned(), on(MENTIONED), f),
            (scheduled(), on(SCHEDULED), f),
            (protected(), on(PROTECTED), f),
            (media_group(), on(MEDIA_GROUP), f),
            (service(), on(SERVICE), f),
            (outgoing(), on(OUTGOING), f),
            (incoming(), f, on(OUTGOING)),
            (me(), on(SENDER_ME), f),
            (bot(), on(SENDER_BOT), f),
            (service_kind([ServiceKind::PinnedMessage]), Facts { service: Some(ServiceKind::PinnedMessage), ..f }, f),
            (private(), inside(ChatType::Bot), inside(ChatType::Group)),
            (group(), inside(ChatType::Supergroup), inside(ChatType::Channel)),
            (supergroup(), inside(ChatType::Supergroup), inside(ChatType::Group)),
            (channel(), inside(ChatType::Channel), inside(ChatType::Private)),
            (chat(-100123), Facts { chat_id: -100123, ..f }, Facts { chat_id: 5, ..f }),
            (user([8, 7]), Facts { sender_id: 8, ..f }, Facts { sender_id: 9, ..f }),
            (topic(4), Facts { flags: TOPIC, thread_id: 4, ..f }, Facts { flags: TOPIC, thread_id: 5, ..f }),
            (all(), f, f),
        ];
        for (i, (filter, yes, no)) in cases.iter().enumerate() {
            assert!(hit(filter, *yes), "case {i} should match");
            assert!(i == cases.len() - 1 || !hit(filter, *no), "case {i} should not match");
        }
        let carries = |data: &'static [u8]| Facts { data, ..f };
        let (exact, prefix): (Filter<CallbackQuery>, Filter<CallbackQuery>) = (data("buy"), data_prefix(b"buy:"));
        assert_eq!((hit(&exact, carries(b"buy")), hit(&exact, carries(b"buy:1"))), (true, false));
        assert_eq!((hit(&prefix, carries(b"buy:1")), hit(&prefix, carries(b"sell:1"))), (true, false));
    }

    #[test]
    fn operators_fold_and_keep_order() {
        let f: Filter = text() & reply() & incoming();
        assert!(matches!(f.0, Pred::Flags { set, clear } if set == TEXT | REPLY && clear == OUTGOING));
        let f: Filter = photo() | video() | (private() | channel());
        assert!(matches!(&f.0, Pred::Or(p) if p.len() == 2));
        let f: Filter = !!text();
        assert!(matches!(f.0, Pred::Flags { .. }));
        let f: Filter = create(|_| true) & text() & reply();
        assert!(matches!(&f.0, Pred::And(p) if matches!(p[0], Pred::Custom(_)) && p.len() == 2));
    }

    #[test]
    fn closures_only_decide_what_facts_cannot() {
        let private_chat = Facts { chat: Some(ChatType::Private), ..Facts::default() };
        let f: Filter = !private() & create(|_| true);
        assert_eq!(f.decide(&private_chat), Some(false));
        let f: Filter = private() | create(|_| false);
        assert_eq!(f.decide(&private_chat), Some(true));
        let f: Filter = private() & create(|_| false);
        assert_eq!(f.decide(&private_chat), None);
    }

    #[test]
    fn composed_filters_follow_boolean_logic() {
        type Rule = fn(bool, bool, bool, bool, bool) -> bool;
        let cases: Vec<(Filter, Rule)> = vec![
            (text() & reply() & incoming(), |t, r, o, _, _| t && r && !o),
            (!(text() & incoming()), |t, _, o, _, _| !(t && !o)),
            (incoming() & outgoing(), |_, _, _, _, _| false),
            (text() & !text(), |_, _, _, _, _| false),
            ((text() | reply()) & !incoming(), |t, r, o, _, _| (t || r) && o),
            (private() | channel() | text(), |t, _, _, p, c| p || c || t),
            (!(private() | channel()) & !text(), |t, _, _, p, c| !(p || c) && !t),
            (!!reply() & (all() | text()), |_, r, _, _, _| r),
            (!all() | (channel() & outgoing()), |_, _, o, _, c| c && o),
        ];
        for bits in 0..8u32 {
            let (t, r, o) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
            let flags = [(t, TEXT), (r, REPLY), (o, OUTGOING)].iter().filter(|(on, _)| *on).fold(0, |f, (_, b)| f | b);
            for chat in [ChatType::Private, ChatType::Group, ChatType::Channel] {
                let facts = Facts { flags, chat: Some(chat), ..Facts::default() };
                let (p, c) = (chat == ChatType::Private, chat == ChatType::Channel);
                for (i, (filter, rule)) in cases.iter().enumerate() {
                    assert_eq!(hit(filter, facts), rule(t, r, o, p, c), "case {i}, flags {flags:b}, {chat:?}");
                }
            }
        }
    }

    #[test]
    fn closures_run_only_when_needed() {
        let client = Client::offline();
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = {
            let calls = calls.clone();
            create(move |_: &Message| {
                calls.fetch_add(1, Ordering::Relaxed);
                true
            })
        };
        let (plain, empty) = (message(&client, 7, "hi", None), message(&client, 7, "", None));
        assert!(!(!all() & counted.clone()).check(&plain));
        assert!((all() | counted.clone()).check(&plain));
        assert!(!(text() & counted.clone()).check(&empty));
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!((all() & text() & counted).check(&plain));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn check_reads_a_parsed_message() {
        let client = Client::offline();
        client.0.me.lock().unwrap().username = Some("mybot".into());
        let start: Filter = command(["start", "help"]);
        let cases = [("/START@MyBot now", true), ("/help", true), ("/start@otherbot", false), ("/stop", false)];
        for (input, matches) in cases {
            assert_eq!(start.check(&message(&client, 7, input, None)), matches, "{input}");
        }

        let (dm, group) = (message(&client, 7, "hi", None), message(&client, -5, "hi", None));
        let private: Filter = private();
        assert_eq!((private.check(&dm), private.check(&group)), (true, false));
        let dice = types::MessageMediaDice { value: 6, emoticon: "🎲".into(), game_outcome: None };
        let dice = Some(base::MessageMedia::MessageMediaDice(Box::new(dice)));
        let captioned = message(&client, 7, "/start", dice.clone());
        let (text, caption): (Filter, Filter) = (text(), caption());
        let shapes = |m: &Message| (text.check(m), caption.check(m), start.check(m));
        assert_eq!(shapes(&dm), (true, false, false));
        assert_eq!(shapes(&captioned), (false, true, true));
        assert_eq!(shapes(&message(&client, 7, "", dice)), (false, false, false));
        assert_eq!(shapes(&message(&client, 7, "", None)), (false, false, false));
    }
}
