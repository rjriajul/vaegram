use crate::dispatcher::Entities;
use crate::enums::MessageEntityType;
use crate::raw::base;
use crate::types::User;

/// A formatted span of message text, such as bold text, a link or a mention.
///
/// Offsets and lengths are in UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MessageEntity {
    /// What the entity is, such as bold text or a link.
    pub kind: MessageEntityType,
    /// Start of the entity in the text, in UTF-16 code units.
    pub offset: i32,
    /// Length of the entity, in UTF-16 code units.
    pub length: i32,
    /// Target of a text link. `None` for other kinds.
    pub url: Option<String>,
    /// The mentioned user of a text mention. `None` for other kinds.
    pub user: Option<User>,
    /// Programming language of a pre-formatted block. `None` if the block has none or the entity is another kind.
    pub language: Option<String>,
    /// Id of the custom emoji. `None` for other kinds.
    pub custom_emoji_id: Option<i64>,
    /// Unix time shown by a date-time entity. `None` for other kinds.
    pub unix_time: Option<i32>,
    /// For [`MessageEntityType::DateTime`], in the Bot API's `r|w?[dD]?[tT]?` syntax.
    pub date_time_format: Option<String>,
    /// For [`MessageEntityType::DiffReplace`], the text it replaces.
    pub old_text: Option<String>,
}

impl MessageEntity {
    /// Creates an entity of `kind` covering `length` UTF-16 code units from `offset`, with no kind-specific data.
    ///
    /// Writing a [`TextMention`](MessageEntityType::TextMention), [`CustomEmoji`](MessageEntityType::CustomEmoji) or
    /// [`DateTime`](MessageEntityType::DateTime) entity fails with `Error::InvalidArgument` until its
    /// [`user`](Self::user), [`custom_emoji_id`](Self::custom_emoji_id) or [`unix_time`](Self::unix_time) is set.
    pub fn new(kind: MessageEntityType, offset: i32, length: i32) -> Self {
        MessageEntity {
            kind,
            offset,
            length,
            url: None,
            user: None,
            language: None,
            custom_emoji_id: None,
            unix_time: None,
            date_time_format: None,
            old_text: None,
        }
    }

    /// Sets the target of a text link.
    #[must_use]
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Sets the mentioned user of a text mention.
    #[must_use]
    pub fn user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }

    /// Sets the programming language of a pre-formatted block.
    #[must_use]
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Sets the id of a custom emoji.
    #[must_use]
    pub fn custom_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.custom_emoji_id = Some(custom_emoji_id);
        self
    }

    /// Sets the Unix time a date-time entity shows.
    #[must_use]
    pub fn unix_time(mut self, unix_time: i32) -> Self {
        self.unix_time = Some(unix_time);
        self
    }

    /// Sets how a [`MessageEntityType::DateTime`] shows its time, in the Bot API's `r|w?[dD]?[tT]?` syntax.
    #[must_use]
    pub fn date_time_format(mut self, date_time_format: impl Into<String>) -> Self {
        self.date_time_format = Some(date_time_format.into());
        self
    }

    /// Sets the text a [`MessageEntityType::DiffReplace`] replaces.
    #[must_use]
    pub fn old_text(mut self, old_text: impl Into<String>) -> Self {
        self.old_text = Some(old_text.into());
        self
    }

    pub(crate) fn parse_all(raw: &[base::MessageEntity], entities: &Entities) -> Option<Vec<MessageEntity>> {
        (!raw.is_empty()).then(|| raw.iter().map(|e| MessageEntity::parse(e, entities)).collect())
    }

    pub(crate) fn parse(raw: &base::MessageEntity, entities: &Entities) -> MessageEntity {
        use MessageEntityType as T;
        use base::MessageEntity as E;
        let (kind, offset, length) = match raw {
            E::MessageEntityUnknown(e) => (T::Unknown, e.offset, e.length),
            E::MessageEntityMention(e) => (T::Mention, e.offset, e.length),
            E::MessageEntityHashtag(e) => (T::Hashtag, e.offset, e.length),
            E::MessageEntityBotCommand(e) => (T::BotCommand, e.offset, e.length),
            E::MessageEntityUrl(e) => (T::Url, e.offset, e.length),
            E::MessageEntityEmail(e) => (T::Email, e.offset, e.length),
            E::MessageEntityBold(e) => (T::Bold, e.offset, e.length),
            E::MessageEntityItalic(e) => (T::Italic, e.offset, e.length),
            E::MessageEntityCode(e) => (T::Code, e.offset, e.length),
            E::MessageEntityPre(e) => (T::Pre, e.offset, e.length),
            E::MessageEntityTextUrl(e) => (T::TextLink, e.offset, e.length),
            E::MessageEntityMentionName(e) => (T::TextMention, e.offset, e.length),
            E::InputMessageEntityMentionName(e) => (T::TextMention, e.offset, e.length),
            E::MessageEntityPhone(e) => (T::PhoneNumber, e.offset, e.length),
            E::MessageEntityCashtag(e) => (T::Cashtag, e.offset, e.length),
            E::MessageEntityUnderline(e) => (T::Underline, e.offset, e.length),
            E::MessageEntityStrike(e) => (T::Strikethrough, e.offset, e.length),
            E::MessageEntityBankCard(e) => (T::BankCard, e.offset, e.length),
            E::MessageEntitySpoiler(e) => (T::Spoiler, e.offset, e.length),
            E::MessageEntityCustomEmoji(e) => (T::CustomEmoji, e.offset, e.length),
            E::MessageEntityBlockquote(e) if e.collapsed => (T::ExpandableBlockquote, e.offset, e.length),
            E::MessageEntityBlockquote(e) => (T::Blockquote, e.offset, e.length),
            E::MessageEntityFormattedDate(e) => (T::DateTime, e.offset, e.length),
            E::MessageEntityDiffInsert(e) => (T::DiffInsert, e.offset, e.length),
            E::MessageEntityDiffReplace(e) => (T::DiffReplace, e.offset, e.length),
            E::MessageEntityDiffDelete(e) => (T::DiffDelete, e.offset, e.length),
            E::MessageEntityTonAddress(e) => (T::TonAddress, e.offset, e.length),
        };
        let mut entity = MessageEntity {
            kind,
            offset,
            length,
            url: None,
            user: None,
            language: None,
            custom_emoji_id: None,
            unix_time: None,
            date_time_format: None,
            old_text: None,
        };
        match raw {
            E::MessageEntityPre(e) if !e.language.is_empty() => entity.language = Some(e.language.clone()),
            E::MessageEntityTextUrl(e) => entity.url = Some(e.url.clone()),
            E::MessageEntityMentionName(e) => entity.user = Some(User::parse(e.user_id, entities)),
            E::InputMessageEntityMentionName(e) => {
                if let base::InputUser::InputUser(u) = &e.user_id {
                    entity.user = Some(User::parse(u.user_id, entities));
                }
            }
            E::MessageEntityCustomEmoji(e) => entity.custom_emoji_id = Some(e.document_id),
            E::MessageEntityDiffReplace(e) => entity.old_text = Some(e.old_text.clone()),
            E::MessageEntityFormattedDate(e) => {
                entity.unix_time = Some(e.date);
                let format: String = if e.relative {
                    "r".into()
                } else {
                    [
                        (e.day_of_week, 'w'),
                        (e.long_date, 'D'),
                        (e.short_date && !e.long_date, 'd'),
                        (e.long_time, 'T'),
                        (e.short_time && !e.long_time, 't'),
                    ]
                    .into_iter()
                    .filter_map(|(set, letter)| set.then_some(letter))
                    .collect()
                };
                entity.date_time_format = (!format.is_empty()).then_some(format);
            }
            _ => {}
        }
        entity
    }
}

#[cfg(test)]
mod tests {
    use super::MessageEntity;
    use crate::dispatcher::Entities;
    use crate::enums::MessageEntityType as T;
    use crate::raw::{base, types};

    fn date(relative: bool, long_date: bool, short_time: bool) -> base::MessageEntity {
        base::MessageEntity::MessageEntityFormattedDate(Box::new(types::MessageEntityFormattedDate {
            relative,
            short_time,
            long_time: false,
            short_date: true,
            long_date,
            day_of_week: true,
            offset: 0,
            length: 5,
            date: 1_700_000_000,
        }))
    }

    #[test]
    fn maps_entities() {
        let raw = [
            date(false, true, true),
            date(true, false, false),
            base::MessageEntity::MessageEntityBlockquote(Box::new(types::MessageEntityBlockquote {
                collapsed: true,
                offset: 1,
                length: 2,
            })),
            base::MessageEntity::MessageEntityPre(Box::new(types::MessageEntityPre {
                offset: 3,
                length: 4,
                language: String::new(),
            })),
            base::MessageEntity::MessageEntityMentionName(Box::new(types::MessageEntityMentionName {
                offset: 5,
                length: 6,
                user_id: 42,
            })),
        ];
        let parsed = MessageEntity::parse_all(&raw, &Entities::default()).unwrap();
        let kinds: Vec<_> = parsed.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [T::DateTime, T::DateTime, T::ExpandableBlockquote, T::Pre, T::TextMention]);
        let formats: Vec<_> = parsed.iter().map(|e| e.date_time_format.as_deref()).collect();
        assert_eq!(formats[..2], [Some("wDt"), Some("r")]);
        assert_eq!((parsed[0].unix_time, parsed[2].offset, parsed[2].length), (Some(1_700_000_000), 1, 2));
        assert_eq!((parsed[3].language.as_deref(), parsed[4].user.as_ref().map(|u| u.id)), (None, Some(42)));
        assert_eq!(MessageEntity::parse_all(&[], &Entities::default()), None);
    }

    #[test]
    fn new_matches_a_parsed_entity() {
        let raw = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 1, length: 3 });
        assert_eq!(MessageEntity::new(T::Bold, 1, 3), MessageEntity::parse(&raw, &Entities::default()));
        let link = types::MessageEntityTextUrl { offset: 2, length: 4, url: "https://a.b".into() };
        let raw = base::MessageEntity::MessageEntityTextUrl(Box::new(link));
        let built = MessageEntity::new(T::TextLink, 2, 4).url("https://a.b");
        assert_eq!(built, MessageEntity::parse(&raw, &Entities::default()));
        assert!(built.user.is_none() && built.custom_emoji_id.is_none());
    }
}
