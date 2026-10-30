use crate::dispatcher::Entities;
use crate::enums::MessageEntityType;
use crate::raw::base;
use crate::types::User;

/// Offsets and lengths are in UTF-16 code units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEntity {
    pub kind: MessageEntityType,
    pub offset: i32,
    pub length: i32,
    pub url: Option<String>,
    pub user: Option<User>,
    pub language: Option<String>,
    pub custom_emoji_id: Option<i64>,
    pub unix_time: Option<i32>,
    /// For [`MessageEntityType::DateTime`], in the Bot API's `r|w?[dD]?[tT]?` syntax.
    pub date_time_format: Option<String>,
    /// For [`MessageEntityType::DiffReplace`], the text it replaces.
    pub old_text: Option<String>,
}

impl MessageEntity {
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
}
