mod facts;
mod pred;
#[cfg(feature = "regex")]
mod regex;
mod subjects;

use std::ops::{BitAnd, BitOr, Not};
use std::sync::Arc;

#[cfg(feature = "regex")]
pub use self::regex::{Searchable, matches, regex};
pub(crate) use facts::{Facts, PeerKinds};
use pred::{Pred, Prefixes};
pub(crate) use subjects::private::Inspect;
pub use subjects::{HasChat, HasData, HasMedia, HasSender, HasText, Subject};

use crate::enums::{ChatType, MediaKind, ServiceKind};
use crate::types::Message;

/// A test a `T` update passes or fails, given to a `Client::on_*` registration. Combine filters with `&`, `|` and
/// `!`. On messages and callback queries, built-in filters run before the update is parsed, so a miss costs no
/// parse; a [`create`] closure always needs the parsed update.
pub struct Filter<T = Message>(Pred<T>);

impl<T> Clone for Filter<T> {
    fn clone(&self) -> Self {
        Filter(self.0.clone())
    }
}

impl<T: Subject + Inspect> Filter<T> {
    /// Tests `item` outside a handler, the way dispatch would, against the account the client knows itself as.
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

/// Command names for `command`: `"start"` or `["ban", "kick"]`. `command`, `command_with` and
/// `command_with_prefixes` match them without case, in any script; `command_case_sensitive` needs the exact case.
pub trait CommandNames {
    /// The names, in the form a command filter keeps them.
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

/// Passes every update; for a handler that wants them all.
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

/// Text that captions media. A link preview isn't media here: text with one is [`text`].
pub fn caption<T: HasMedia>() -> Filter<T> {
    flags(facts::CAPTION)
}

/// `/name`, or `/name@us`; never `/name@another_bot`. Read it in the handler with `Message::command`.
pub fn command<T: HasText>(names: impl CommandNames) -> Filter<T> {
    command_with("/", names)
}

/// Like [`command`], but any character of `prefixes` starts a command: `"/!"` takes `/start` and `!start`.
///
/// # Panics
///
/// If `prefixes` is empty.
pub fn command_with<T: HasText>(prefixes: &str, names: impl CommandNames) -> Filter<T> {
    command_filter(prefixes, names, false)
}

/// Like [`command`], but any of the `prefixes` strings starts a command: `[".", "!x"]` takes `.start` and
/// `!xstart`. When several prefixes start the text, the longest one is taken, so with `["!", "!x"]` the text
/// `!xstart` is the command `start`. Read it in the handler with `Message::command_with_prefixes`.
///
/// # Panics
///
/// If `prefixes` is empty or holds an empty string.
pub fn command_with_prefixes<T: HasText>(
    prefixes: impl IntoIterator<Item = impl AsRef<str>>,
    names: impl CommandNames,
) -> Filter<T> {
    let prefixes: Box<[Box<str>]> = prefixes.into_iter().map(|p| p.as_ref().into()).collect();
    assert!(!prefixes.is_empty(), "command prefixes can't be empty; use [\"/\"] for bot commands");
    assert!(prefixes.iter().all(|p| !p.is_empty()), "a command prefix can't be an empty string");
    Filter(Pred::Commands { prefixes: Prefixes::Strings(prefixes), names: names.names(), case_sensitive: false })
}

/// Like `command_with`, but the name must have the case it was given: `/Start` isn't `/start`.
pub fn command_case_sensitive<T: HasText>(prefixes: &str, names: impl CommandNames) -> Filter<T> {
    command_filter(prefixes, names, true)
}

fn command_filter<T: HasText>(prefixes: &str, names: impl CommandNames, case_sensitive: bool) -> Filter<T> {
    assert!(!prefixes.is_empty(), "command prefixes can't be empty; use \"/\" for bot commands");
    Filter(Pred::Commands { prefixes: Prefixes::Chars(prefixes.into()), names: names.names(), case_sensitive })
}

/// Text, caption or inline query that starts with `prefix`, compared case-sensitively.
pub fn text_starts_with<T: HasText>(prefix: &str) -> Filter<T> {
    Filter(Pred::TextPrefix(prefix.into()))
}

/// Any media except a link preview, including media of a kind vaegram doesn't know.
pub fn media<T: HasMedia>() -> Filter<T> {
    Filter(Pred::Media(!(1 << MediaKind::WebPage as u8)))
}

macro_rules! media_filters {
    ($($(#[$doc:meta])* $name:ident => $kind:ident),* $(,)?) => {$(
        $(#[$doc])*
        pub fn $name<T: HasMedia>() -> Filter<T> {
            Filter(Pred::Media(1 << MediaKind::$kind as u8))
        }
    )*};
}

media_filters! {
    /// A photo.
    photo => Photo,
    /// A video; animations and round video notes don't count.
    video => Video,
    /// An animation, such as a GIF.
    animation => Animation,
    /// An audio file, such as music; voice notes don't count.
    audio => Audio,
    /// A voice note.
    voice => Voice,
    /// A round video message.
    video_note => VideoNote,
    /// A file that is none of the other media kinds.
    document => Document,
    /// A sticker.
    sticker => Sticker,
    /// A live photo.
    live_photo => LivePhoto,
    /// A shared contact.
    contact => Contact,
    /// A location, live or not; a venue doesn't count.
    location => Location,
    /// A venue: a location with a title and address.
    venue => Venue,
    /// A dice or another animated emoji that lands on a random result.
    dice => Dice,
    /// A poll.
    poll => Poll,
    /// A game.
    game => Game,
    /// An invoice.
    invoice => Invoice,
    /// A checklist.
    checklist => Checklist,
    /// A forwarded story.
    story => Story,
    /// A giveaway announcement only; its results are `giveaway_winners`.
    giveaway => Giveaway,
    /// The results of a giveaway only; the announcement is `giveaway`.
    giveaway_winners => GiveawayWinners,
    /// Media behind a Stars paywall.
    paid_media => PaidMedia,
    /// A link preview with no other media; [`media`] leaves these out.
    web_page => WebPage,
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
    /// A reply to another message. Pointing only at a forum topic's root, or a pin's header, doesn't count.
    reply: HasMedia => REPLY,
    /// Forwarded, including automatic forwards from a linked channel.
    forwarded: HasMedia => FORWARDED,
    /// Sent through an inline bot.
    via_bot: HasMedia => VIA_BOT,
    /// Mentions this account (Telegram's `mentioned` flag).
    mentioned: HasMedia => MENTIONED,
    /// Sent by the server at a time the sender had scheduled.
    from_scheduled: HasMedia => FROM_SCHEDULED,
    /// Not sent yet: it waits for its date, as `get_scheduled_messages` returns it. Messages that reach handlers are
    /// never in this state, so this only tells apart the parsed messages you fetched yourself.
    scheduled: HasMedia => SCHEDULED,
    /// Protected content: it can't be forwarded or saved.
    protected: HasMedia => PROTECTED,
    /// Part of an album.
    media_group: HasMedia => MEDIA_GROUP,
    /// A service message, such as a member joining or a pin. Narrow it with [`service_kind`].
    service: HasMedia => SERVICE,
    /// Sent by this account. Updates that aren't messages never are.
    outgoing: HasChat => OUTGOING,
    /// Sent or done by this account.
    me: HasSender => SENDER_ME,
    /// Sent or done by a bot.
    bot: HasSender => SENDER_BOT,
    /// Sent by a chat rather than a user: a channel post, an anonymous admin, or a chat reacting or voting.
    sender_chat: HasSender => SENDER_CHAT,
    /// Replies quoting part of the replied-to message.
    quote: HasMedia => QUOTE,
    /// Carries an inline keyboard.
    inline_keyboard: HasMedia => INLINE_KEYBOARD,
    /// Carries a reply keyboard; a keyboard removal or a forced reply doesn't count.
    reply_keyboard: HasMedia => REPLY_KEYBOARD,
    /// A photo or document hidden under a spoiler.
    media_spoiler: HasMedia => MEDIA_SPOILER,
    /// A channel post forwarded automatically into its linked discussion group.
    linked_channel: HasMedia => LINKED_CHANNEL,
    /// A live location; a static location doesn't count.
    live_location: HasMedia => LIVE_LOCATION,
    /// A photo or document that disappears after it's opened.
    self_destruction: HasMedia => SELF_DESTRUCTION,
    /// A message the sender paid Stars to send.
    paid_message: HasMedia => PAID_MESSAGE,
    /// Sent through a business connection. Only business message routes and their parsed messages see these.
    business: HasMedia => BUSINESS,
    /// In a forum supergroup.
    forum: HasChat => FORUM,
    /// In a chat where we have admin rights.
    admin: HasChat => ADMIN,
}

/// Not sent by this account: the exact opposite of [`outgoing`].
pub fn incoming<T: HasChat>() -> Filter<T> {
    Filter(Pred::Flags { set: 0, clear: facts::OUTGOING })
}

/// In the forum topic whose root message is `id`, including the message that created it.
pub fn topic<T: HasMedia>(id: i32) -> Filter<T> {
    flags(facts::TOPIC) & Filter(Pred::Thread(id))
}

const _: () = assert!(ServiceKind::Unknown as u32 <= 127);

/// A service message of one of `kinds`.
pub fn service_kind<T: HasMedia>(kinds: impl IntoIterator<Item = ServiceKind>) -> Filter<T> {
    Filter(Pred::Service(kinds.into_iter().fold(0u128, |mask, k| mask | 1 << k as u8)))
}

macro_rules! service_filters {
    ($($(#[$doc:meta])* $name:ident => [$($kind:ident),+]),* $(,)?) => {$(
        $(#[$doc])*
        pub fn $name<T: HasMedia>() -> Filter<T> {
            service_kind([$(ServiceKind::$kind),+])
        }
    )*};
}

service_filters! {
    /// Members were added to the chat, or joined it.
    new_chat_members => [NewChatMembers],
    /// A member left or was removed.
    left_chat_member => [LeftChatMember],
    /// The chat's title changed.
    new_chat_title => [NewChatTitle],
    /// The chat's photo changed.
    new_chat_photo => [NewChatPhoto],
    /// The chat's photo was removed.
    delete_chat_photo => [DeleteChatPhoto],
    /// A basic group was created.
    group_chat_created => [GroupChatCreated],
    /// A supergroup was created.
    supergroup_chat_created => [SupergroupChatCreated],
    /// A channel was created.
    channel_chat_created => [ChannelChatCreated],
    /// The group was upgraded to a supergroup; the message is in the old group.
    migrate_to_chat_id => [MigrateToChatId],
    /// The supergroup was upgraded from a group; the message is in the new supergroup.
    migrate_from_chat_id => [MigrateFromChatId],
    /// A message was pinned.
    pinned_message => [PinnedMessage],
    /// Someone scored in a game.
    game_high_score => [GameHighScore],
    /// A video chat started.
    video_chat_started => [VideoChatStarted],
    /// A video chat ended.
    video_chat_ended => [VideoChatEnded],
    /// Members were invited to a video chat.
    video_chat_members_invited => [VideoChatParticipantsInvited],
    /// A payment went through.
    successful_payment => [SuccessfulPayment],
    /// The user shared users with the bot through a request button.
    users_shared => [UsersShared],
    /// The user shared a chat with the bot through a request button.
    chat_shared => [ChatShared],
    /// A Premium gift code was received.
    gift_code => [PremiumGiftCode],
    /// A gift was received, regular or upgraded.
    gift => [Gift, UniqueGift],
}

/// An offer to buy an upgraded gift that nobody has answered yet.
pub fn gift_offer<T: HasMedia>() -> Filter<T> {
    service_kind([ServiceKind::UpgradedGiftPurchaseOffer])
        & Filter(Pred::Flags { set: 0, clear: facts::OFFER_ACCEPTED | facts::OFFER_REJECTED })
}

/// An offer to buy an upgraded gift that was accepted.
pub fn gift_offer_accepted<T: HasMedia>() -> Filter<T> {
    service_kind([ServiceKind::UpgradedGiftPurchaseOffer]) & flags(facts::OFFER_ACCEPTED)
}

/// An offer to buy an upgraded gift that was turned down or left to run out.
pub fn gift_offer_rejected<T: HasMedia>() -> Filter<T> {
    (service_kind([ServiceKind::UpgradedGiftPurchaseOffer]) & flags(facts::OFFER_REJECTED))
        | service_kind([ServiceKind::UpgradedGiftPurchaseOfferRejected])
}

/// Private chats, with people or bots; [`direct`] leaves bots out.
pub fn private<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Private, ChatType::Bot])
}

/// Private chats with people. Unlike `private`, it leaves out bots, and it isn't a channel's direct messages chat.
pub fn direct<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Private])
}

/// Basic groups and supergroups.
pub fn group<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Group, ChatType::Supergroup])
}

/// Supergroups only, forums included; basic groups don't count.
pub fn supergroup<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Supergroup])
}

/// Broadcast channels; supergroups don't count.
pub fn channel<T: HasChat>() -> Filter<T> {
    chat_kinds(&[ChatType::Channel])
}

/// In one of these chats, by Bot-API id: positive for users, negative for basic groups, `-100…` for channels and
/// supergroups.
pub fn chat<T: HasChat>(ids: impl Into<Ids>) -> Filter<T> {
    Filter(Pred::Chats(ids.into().0))
}

/// Sent or done by one of these users. A chat acting as itself, such as a channel or an anonymous admin, matches
/// its Bot-API chat id.
pub fn user<T: HasSender>(ids: impl Into<Ids>) -> Filter<T> {
    Filter(Pred::Users(ids.into().0))
}

/// Callback data equal to `value`, byte for byte.
pub fn data<T: HasData>(value: impl AsRef<[u8]>) -> Filter<T> {
    Filter(Pred::Data(value.as_ref().into()))
}

/// Callback data that starts with `prefix`.
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
    use crate::types::{CallbackQuery, Story};
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
            (giveaway(), holds(MediaKind::Giveaway), holds(MediaKind::GiveawayWinners)),
            (giveaway_winners(), holds(MediaKind::GiveawayWinners), holds(MediaKind::Giveaway)),
            (video_note(), holds(MediaKind::VideoNote), f),
            (reply(), on(REPLY), f),
            (forwarded(), on(FORWARDED), f),
            (via_bot(), on(VIA_BOT), f),
            (mentioned(), on(MENTIONED), f),
            (from_scheduled(), on(FROM_SCHEDULED), on(SCHEDULED)),
            (scheduled(), on(SCHEDULED), on(FROM_SCHEDULED)),
            (protected(), on(PROTECTED), f),
            (media_group(), on(MEDIA_GROUP), f),
            (service(), on(SERVICE), f),
            (outgoing(), on(OUTGOING), f),
            (incoming(), f, on(OUTGOING)),
            (me(), on(SENDER_ME), f),
            (bot(), on(SENDER_BOT), f),
            (sender_chat(), on(SENDER_CHAT), on(SENDER_BOT)),
            (quote(), on(QUOTE | REPLY), on(REPLY)),
            (inline_keyboard(), on(INLINE_KEYBOARD), on(REPLY_KEYBOARD)),
            (reply_keyboard(), on(REPLY_KEYBOARD), on(INLINE_KEYBOARD)),
            (media_spoiler(), on(MEDIA_SPOILER), f),
            (linked_channel(), on(LINKED_CHANNEL | FORWARDED), on(FORWARDED)),
            (live_location(), on(LIVE_LOCATION), f),
            (self_destruction(), on(SELF_DESTRUCTION), f),
            (paid_message(), on(PAID_MESSAGE), f),
            (business(), on(BUSINESS), f),
            (forum(), on(FORUM), inside(ChatType::Supergroup)),
            (admin(), on(ADMIN), inside(ChatType::Group)),
            (service_kind([ServiceKind::PinnedMessage]), Facts { service: Some(ServiceKind::PinnedMessage), ..f }, f),
            (private(), inside(ChatType::Bot), inside(ChatType::Group)),
            (direct(), inside(ChatType::Private), inside(ChatType::Bot)),
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
    fn service_kind_masks_cover_high_kinds() {
        let f = Facts::default();
        let is = |k| Facts { service: Some(k), ..f };
        let high: Filter = service_kind([ServiceKind::ChatSetTheme]);
        assert!(hit(&high, is(ServiceKind::ChatSetTheme)));
        assert!(!hit(&high, is(ServiceKind::LeftChatMember)));
        let both: Filter = service_kind([ServiceKind::LeftChatMember, ServiceKind::ChatSetTheme]);
        assert!(hit(&both, is(ServiceKind::LeftChatMember)) && hit(&both, is(ServiceKind::ChatSetTheme)));
        assert!(!hit(&both, is(ServiceKind::PinnedMessage)));
        let top: Filter = service_kind([ServiceKind::PassportDataReceived]);
        assert!(hit(&top, is(ServiceKind::PassportDataReceived)));
        assert!(!hit(&top, is(ServiceKind::AutoDeleteTimerChanged)));
    }

    #[test]
    fn every_named_filter_matches_its_kind_only() {
        use ServiceKind as K;
        let cases: Vec<(Filter, &[ServiceKind])> = vec![
            (new_chat_members(), &[K::NewChatMembers]),
            (left_chat_member(), &[K::LeftChatMember]),
            (new_chat_title(), &[K::NewChatTitle]),
            (new_chat_photo(), &[K::NewChatPhoto]),
            (delete_chat_photo(), &[K::DeleteChatPhoto]),
            (group_chat_created(), &[K::GroupChatCreated]),
            (supergroup_chat_created(), &[K::SupergroupChatCreated]),
            (channel_chat_created(), &[K::ChannelChatCreated]),
            (migrate_to_chat_id(), &[K::MigrateToChatId]),
            (migrate_from_chat_id(), &[K::MigrateFromChatId]),
            (pinned_message(), &[K::PinnedMessage]),
            (game_high_score(), &[K::GameHighScore]),
            (video_chat_started(), &[K::VideoChatStarted]),
            (video_chat_ended(), &[K::VideoChatEnded]),
            (video_chat_members_invited(), &[K::VideoChatParticipantsInvited]),
            (successful_payment(), &[K::SuccessfulPayment]),
            (users_shared(), &[K::UsersShared]),
            (chat_shared(), &[K::ChatShared]),
            (gift_code(), &[K::PremiumGiftCode]),
            (gift(), &[K::Gift, K::UniqueGift]),
        ];
        let every: Vec<ServiceKind> = cases.iter().flat_map(|(_, kinds)| kinds.iter().copied()).collect();
        let f = Facts::default();
        for (i, (filter, kinds)) in cases.iter().enumerate() {
            for kind in &every {
                let facts = Facts { service: Some(*kind), ..f };
                assert_eq!(hit(filter, facts), kinds.contains(kind), "case {i} against {kind:?}");
            }
            assert!(!hit(filter, f), "case {i} matched a message that isn't a service message");
        }
    }

    #[test]
    fn gift_offer_filters_follow_the_offer_state() {
        let f = Facts::default();
        let offer = |flags| Facts { flags, service: Some(ServiceKind::UpgradedGiftPurchaseOffer), ..f };
        let gone = Facts { service: Some(ServiceKind::UpgradedGiftPurchaseOfferRejected), ..f };
        let facts = [offer(0), offer(OFFER_ACCEPTED), offer(OFFER_REJECTED), gone, f];
        let cases: Vec<(Filter, [bool; 5])> = vec![
            (gift_offer(), [true, false, false, false, false]),
            (gift_offer_accepted(), [false, true, false, false, false]),
            (gift_offer_rejected(), [false, false, true, true, false]),
        ];
        for (i, (filter, expected)) in cases.iter().enumerate() {
            let got: Vec<bool> = facts.iter().map(|facts| hit(filter, *facts)).collect();
            assert_eq!(got, expected, "case {i}");
        }
    }

    #[test]
    fn giveaway_no_longer_matches_results() {
        let f = Facts::default();
        let results = Facts { media: Some(MediaKind::GiveawayWinners), ..f };
        let (announcement, winners, any): (Filter, Filter, Filter) = (giveaway(), giveaway_winners(), media());
        assert!(!hit(&announcement, results));
        assert!(hit(&winners, results) && hit(&any, results));
        assert!(!hit(&winners, Facts { media: Some(MediaKind::Giveaway), ..f }));
    }

    #[test]
    fn case_sensitive_commands_need_the_exact_case() {
        let says = |text| Facts { text, ..Facts::default() };
        let exact: Filter = command_case_sensitive("/", ["Start", "stop"]);
        assert!(hit(&exact, says("/Start now")) && hit(&exact, says("/stop")));
        assert!(!hit(&exact, says("/start")) && !hit(&exact, says("/STOP")));
        let loose: Filter = command_with("/", "Start");
        assert!(hit(&loose, says("/start")) && hit(&loose, says("/START")));
    }

    #[test]
    fn cyrillic_commands_match_without_case() {
        let says = |text| Facts { text, ..Facts::default() };
        let f: Filter = command_with("/", ["Привет", "ǅ"]);
        assert!(hit(&f, says("/привет")) && hit(&f, says("/ПРИВЕТ мир")) && hit(&f, says("/ǆ")));
        assert!(!hit(&f, says("/приветик")));
        let exact: Filter = command_case_sensitive("/", "Привет");
        assert!(hit(&exact, says("/Привет")) && !hit(&exact, says("/привет")));
    }

    #[test]
    fn string_prefixes_take_the_longest_match() {
        let says = |text| Facts { text, me: Some("Bot"), ..Facts::default() };
        let f: Filter = command_with_prefixes([".", "!x"], ["ban", "Start"]);
        assert!(hit(&f, says(".ban 1")) && hit(&f, says("!xSTART@bot now")));
        assert!(!hit(&f, says("!ban")) && !hit(&f, says("/ban")) && !hit(&f, says("!xstart@other")));
        assert!(!hit(&f, says("x.ban")) && !hit(&f, says(". ban")) && !hit(&f, says("!x")));
        // `!xban` is `ban` after `!x`, never `xban` after `!`.
        let both: Filter = command_with_prefixes(vec!["!".to_owned(), "!x".to_owned()], "ban");
        assert!(hit(&both, says("!xban")) && hit(&both, says("!ban")));
        let short: Filter = command_with_prefixes(["!"], "ban");
        assert!(!hit(&short, says("!xban")));
        let xban: Filter = command_with_prefixes(["!", "!x"], "xban");
        assert!(!hit(&xban, says("!xban")));

        let client = Client::offline();
        let read =
            |text| message(&client, 5, text, None).command_with_prefixes(&["!", "!x"]).map(|c| c.name.to_owned());
        assert_eq!((read("!xban 1"), read("!ban"), read("/ban")), (Some("ban".into()), Some("ban".into()), None));
    }

    #[test]
    #[should_panic(expected = "empty string")]
    fn string_prefixes_refuse_an_empty_prefix() {
        let _: Filter = command_with_prefixes([".", ""], "ban");
    }

    #[test]
    #[should_panic(expected = "can't be empty")]
    fn string_prefixes_refuse_an_empty_list() {
        let _: Filter = command_with_prefixes(Vec::<String>::new(), "ban");
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

    #[test]
    fn filter_check_works_on_a_story() {
        let skipped = types::StoryItemSkipped { close_friends: false, live: false, id: 4, date: 1, expire_date: 2 };
        let item = base::StoryItem::StoryItemSkipped(Box::new(skipped));
        let story = Story::parse(&Client::offline(), &get_peer(8), item, &Entities::default()).unwrap();
        let (private, channel, chat): (Filter<Story>, Filter<Story>, Filter<Story>) = (private(), channel(), chat(8));
        assert_eq!((private.check(&story), channel.check(&story), chat.check(&story)), (true, false, true));
        assert!(!outgoing::<Story>().check(&story));
    }
}
