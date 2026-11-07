use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::MessageEntityType;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::MessageEntity;
use crate::utils::get_channel_id;

/// A chat folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Folder {
    /// Id of the folder, as the `Client` folder methods take it.
    pub id: i32,
    /// Title shown on the folder's tab.
    pub title: String,
    /// Entities in `title`, such as custom emoji.
    pub title_entities: Vec<MessageEntity>,
    /// Whether custom emoji in the title stay still.
    pub no_title_animation: bool,
    /// The emoji shown as the folder's icon.
    pub emoji: Option<String>,
    /// Index of the folder's tag color. `None` for no color.
    pub color: Option<i32>,
    /// Ids of the chats pinned at the top of the folder.
    pub pinned_chats: Vec<i64>,
    /// Ids of the chats that always belong to the folder.
    pub included_chats: Vec<i64>,
    /// Ids of the chats that never belong to the folder.
    pub excluded_chats: Vec<i64>,
    /// Whether the folder includes chats with contacts.
    pub contacts: bool,
    /// Whether the folder includes chats with users who aren't contacts.
    pub non_contacts: bool,
    /// Whether the folder includes groups.
    pub groups: bool,
    /// Whether the folder includes channels.
    pub channels: bool,
    /// Whether the folder includes chats with bots.
    pub bots: bool,
    /// Whether the folder leaves out muted chats.
    pub exclude_muted: bool,
    /// Whether the folder leaves out chats with no unread messages.
    pub exclude_read: bool,
    /// Whether the folder leaves out archived chats.
    pub exclude_archived: bool,
    /// Whether the folder is shared through an invite link.
    ///
    /// A shared folder keeps only its pinned and included chats. Saving it drops the exclusions and chat type flags.
    pub is_shareable: bool,
}

impl Folder {
    /// Creates a folder titled `title` that holds no chats yet; add chats or chat types with the setters.
    pub fn new(title: impl Into<String>) -> Self {
        Folder { title: title.into(), ..Folder::default() }
    }

    /// Sets the id of the folder to edit. [`Client::create_folder`] picks its own and ignores this one.
    #[must_use]
    pub fn id(mut self, id: i32) -> Self {
        self.id = id;
        self
    }

    /// Formats the title, such as with custom emoji.
    #[must_use]
    pub fn title_entities(mut self, title_entities: impl IntoIterator<Item = MessageEntity>) -> Self {
        self.title_entities = title_entities.into_iter().collect();
        self
    }

    /// Sets whether custom emoji in the title stay still.
    #[must_use]
    pub fn no_title_animation(mut self, no_title_animation: bool) -> Self {
        self.no_title_animation = no_title_animation;
        self
    }

    /// Sets the emoji shown as the folder's icon.
    #[must_use]
    pub fn emoji(mut self, emoji: impl Into<String>) -> Self {
        self.emoji = Some(emoji.into());
        self
    }

    /// Sets the index of the folder's tag color.
    #[must_use]
    pub fn color(mut self, color: i32) -> Self {
        self.color = Some(color);
        self
    }

    /// Pins these chats at the top of the folder.
    #[must_use]
    pub fn pinned_chats(mut self, pinned_chats: impl IntoIterator<Item = i64>) -> Self {
        self.pinned_chats = pinned_chats.into_iter().collect();
        self
    }

    /// Always puts these chats in the folder.
    #[must_use]
    pub fn included_chats(mut self, included_chats: impl IntoIterator<Item = i64>) -> Self {
        self.included_chats = included_chats.into_iter().collect();
        self
    }

    /// Never puts these chats in the folder.
    #[must_use]
    pub fn excluded_chats(mut self, excluded_chats: impl IntoIterator<Item = i64>) -> Self {
        self.excluded_chats = excluded_chats.into_iter().collect();
        self
    }

    /// Sets whether the folder includes chats with contacts.
    #[must_use]
    pub fn contacts(mut self, contacts: bool) -> Self {
        self.contacts = contacts;
        self
    }

    /// Sets whether the folder includes chats with users who aren't contacts.
    #[must_use]
    pub fn non_contacts(mut self, non_contacts: bool) -> Self {
        self.non_contacts = non_contacts;
        self
    }

    /// Sets whether the folder includes groups.
    #[must_use]
    pub fn groups(mut self, groups: bool) -> Self {
        self.groups = groups;
        self
    }

    /// Sets whether the folder includes channels.
    #[must_use]
    pub fn channels(mut self, channels: bool) -> Self {
        self.channels = channels;
        self
    }

    /// Sets whether the folder includes chats with bots.
    #[must_use]
    pub fn bots(mut self, bots: bool) -> Self {
        self.bots = bots;
        self
    }

    /// Sets whether the folder leaves out muted chats.
    #[must_use]
    pub fn exclude_muted(mut self, exclude_muted: bool) -> Self {
        self.exclude_muted = exclude_muted;
        self
    }

    /// Sets whether the folder leaves out chats with no unread messages.
    #[must_use]
    pub fn exclude_read(mut self, exclude_read: bool) -> Self {
        self.exclude_read = exclude_read;
        self
    }

    /// Sets whether the folder leaves out archived chats.
    #[must_use]
    pub fn exclude_archived(mut self, exclude_archived: bool) -> Self {
        self.exclude_archived = exclude_archived;
        self
    }

    /// Sets whether the folder is shared through an invite link. Saving a shared folder drops its exclusions
    /// and chat type flags.
    #[must_use]
    pub fn is_shareable(mut self, is_shareable: bool) -> Self {
        self.is_shareable = is_shareable;
        self
    }

    pub(crate) fn parse(client: &Client, filter: base::DialogFilter) -> Option<Folder> {
        match filter {
            base::DialogFilter::DialogFilterDefault => None,
            base::DialogFilter::DialogFilter(f) => Some(Folder {
                id: f.id,
                title_entities: title_entities(&f.title),
                title: title(f.title),
                no_title_animation: f.title_noanimate,
                emoji: f.emoticon,
                color: f.color,
                pinned_chats: chat_ids(client, f.pinned_peers),
                included_chats: chat_ids(client, f.include_peers),
                excluded_chats: chat_ids(client, f.exclude_peers),
                contacts: f.contacts,
                non_contacts: f.non_contacts,
                groups: f.groups,
                channels: f.broadcasts,
                bots: f.bots,
                exclude_muted: f.exclude_muted,
                exclude_read: f.exclude_read,
                exclude_archived: f.exclude_archived,
                is_shareable: false,
            }),
            base::DialogFilter::DialogFilterChatlist(f) => Some(Folder {
                id: f.id,
                title_entities: title_entities(&f.title),
                title: title(f.title),
                no_title_animation: f.title_noanimate,
                emoji: f.emoticon,
                color: f.color,
                pinned_chats: chat_ids(client, f.pinned_peers),
                included_chats: chat_ids(client, f.include_peers),
                is_shareable: true,
                ..Folder::default()
            }),
        }
    }

    pub(crate) async fn write(&self, client: &Client) -> Result<base::DialogFilter> {
        let mut entities = Vec::with_capacity(self.title_entities.len());
        for entity in &self.title_entities {
            entities.extend(write_entity(client, entity).await?);
        }
        let title = types::TextWithEntities { text: self.title.clone(), entities };
        let title = base::TextWithEntities::TextWithEntities(Box::new(title));
        let pinned_peers = client.resolve_peers(self.pinned_chats.iter().copied()).await?;
        let include_peers = client.resolve_peers(self.included_chats.iter().copied()).await?;
        if self.is_shareable {
            let folder = types::DialogFilterChatlist {
                has_my_invites: false,
                title_noanimate: self.no_title_animation,
                id: self.id,
                title,
                emoticon: self.emoji.clone(),
                color: self.color,
                pinned_peers,
                include_peers,
            };
            return Ok(base::DialogFilter::DialogFilterChatlist(Box::new(folder)));
        }
        let folder = types::DialogFilter {
            contacts: self.contacts,
            non_contacts: self.non_contacts,
            groups: self.groups,
            broadcasts: self.channels,
            bots: self.bots,
            exclude_muted: self.exclude_muted,
            exclude_read: self.exclude_read,
            exclude_archived: self.exclude_archived,
            title_noanimate: self.no_title_animation,
            id: self.id,
            title,
            emoticon: self.emoji.clone(),
            color: self.color,
            pinned_peers,
            include_peers,
            exclude_peers: client.resolve_peers(self.excluded_chats.iter().copied()).await?,
        };
        Ok(base::DialogFilter::DialogFilter(Box::new(folder)))
    }
}

fn title_entities(title: &base::TextWithEntities) -> Vec<MessageEntity> {
    let base::TextWithEntities::TextWithEntities(t) = title;
    MessageEntity::parse_all(&t.entities, &Entities::default()).unwrap_or_default()
}

/// `None` for a mention of a user this account has never met: the title keeps the text without the link.
async fn write_entity(client: &Client, e: &MessageEntity) -> Result<Option<base::MessageEntity>> {
    use MessageEntityType as T;
    use base::MessageEntity as E;
    let (offset, length) = (e.offset, e.length);
    Ok(Some(match e.kind {
        T::Unknown => E::MessageEntityUnknown(types::MessageEntityUnknown { offset, length }),
        T::Mention => E::MessageEntityMention(types::MessageEntityMention { offset, length }),
        T::Hashtag => E::MessageEntityHashtag(types::MessageEntityHashtag { offset, length }),
        T::BotCommand => E::MessageEntityBotCommand(types::MessageEntityBotCommand { offset, length }),
        T::Url => E::MessageEntityUrl(types::MessageEntityUrl { offset, length }),
        T::Email => E::MessageEntityEmail(types::MessageEntityEmail { offset, length }),
        T::Bold => E::MessageEntityBold(types::MessageEntityBold { offset, length }),
        T::Italic => E::MessageEntityItalic(types::MessageEntityItalic { offset, length }),
        T::Code => E::MessageEntityCode(types::MessageEntityCode { offset, length }),
        T::Pre => {
            let language = e.language.clone().unwrap_or_default();
            E::MessageEntityPre(Box::new(types::MessageEntityPre { offset, length, language }))
        }
        T::TextLink => {
            let url = e.url.clone().unwrap_or_default();
            E::MessageEntityTextUrl(Box::new(types::MessageEntityTextUrl { offset, length, url }))
        }
        T::TextMention => {
            let user = e.user.as_ref().ok_or(Error::InvalidArgument("a text mention needs its user"))?;
            let Some(user_id) = client.resolve_mentioned(user.id).await? else { return Ok(None) };
            E::InputMessageEntityMentionName(Box::new(types::InputMessageEntityMentionName { offset, length, user_id }))
        }
        T::PhoneNumber => E::MessageEntityPhone(types::MessageEntityPhone { offset, length }),
        T::Cashtag => E::MessageEntityCashtag(types::MessageEntityCashtag { offset, length }),
        T::Underline => E::MessageEntityUnderline(types::MessageEntityUnderline { offset, length }),
        T::Strikethrough => E::MessageEntityStrike(types::MessageEntityStrike { offset, length }),
        T::BankCard => E::MessageEntityBankCard(types::MessageEntityBankCard { offset, length }),
        T::Spoiler => E::MessageEntitySpoiler(types::MessageEntitySpoiler { offset, length }),
        T::CustomEmoji => {
            let document_id = e.custom_emoji_id.ok_or(Error::InvalidArgument("a custom emoji needs its id"))?;
            E::MessageEntityCustomEmoji(Box::new(types::MessageEntityCustomEmoji { offset, length, document_id }))
        }
        T::Blockquote | T::ExpandableBlockquote => {
            let collapsed = e.kind == T::ExpandableBlockquote;
            E::MessageEntityBlockquote(Box::new(types::MessageEntityBlockquote { collapsed, offset, length }))
        }
        T::DateTime => {
            let date = e.unix_time.ok_or(Error::InvalidArgument("a date entity needs its time"))?;
            let format = e.date_time_format.as_deref().unwrap_or_default();
            let has = |c| format.contains(c);
            E::MessageEntityFormattedDate(Box::new(types::MessageEntityFormattedDate {
                relative: has('r'),
                short_time: has('t'),
                long_time: has('T'),
                short_date: has('d'),
                long_date: has('D'),
                day_of_week: has('w'),
                offset,
                length,
                date,
            }))
        }
        T::DiffInsert => E::MessageEntityDiffInsert(types::MessageEntityDiffInsert { offset, length }),
        T::DiffReplace => {
            let old_text = e.old_text.clone().unwrap_or_default();
            E::MessageEntityDiffReplace(Box::new(types::MessageEntityDiffReplace { offset, length, old_text }))
        }
        T::DiffDelete => E::MessageEntityDiffDelete(types::MessageEntityDiffDelete { offset, length }),
        T::TonAddress => E::MessageEntityTonAddress(types::MessageEntityTonAddress { offset, length }),
    }))
}

fn title(title: base::TextWithEntities) -> String {
    let base::TextWithEntities::TextWithEntities(t) = title;
    t.text
}

/// Folders carry full input peers, so their access hashes go straight into the peer cache.
fn chat_ids(client: &Client, peers: Vec<base::InputPeer>) -> Vec<i64> {
    let me = client.0.me.lock().unwrap().id;
    let mut cache = client.0.peers.lock().unwrap();
    let mut ids = Vec::with_capacity(peers.len());
    for peer in peers {
        let id = match &peer {
            base::InputPeer::InputPeerEmpty => continue,
            base::InputPeer::InputPeerSelf => me,
            base::InputPeer::InputPeerChat(p) => -p.chat_id,
            base::InputPeer::InputPeerUser(p) => p.user_id,
            base::InputPeer::InputPeerChannel(p) => get_channel_id(p.channel_id),
            base::InputPeer::InputPeerUserFromMessage(p) => p.user_id,
            base::InputPeer::InputPeerChannelFromMessage(p) => get_channel_id(p.channel_id),
        };
        if matches!(peer, base::InputPeer::InputPeerUser(_) | base::InputPeer::InputPeerChannel(_)) {
            cache.insert(id, peer);
        }
        ids.push(id);
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::Folder;
    use crate::Client;
    use crate::enums::MessageEntityType;
    use crate::raw::base;
    use crate::types::MessageEntity;

    #[tokio::test]
    async fn builds_from_new_and_setters() {
        let emoji = MessageEntity::new(MessageEntityType::CustomEmoji, 0, 2).custom_emoji_id(5);
        let folder = Folder::new("Work").id(4).title_entities([emoji]).emoji("💼").groups(true).exclude_read(true);
        let Ok(base::DialogFilter::DialogFilter(f)) = folder.write(&Client::offline()).await else {
            panic!("expected a plain folder");
        };
        let base::TextWithEntities::TextWithEntities(title) = &f.title;
        assert_eq!((f.id, title.text.as_str(), f.emoticon.as_deref()), (4, "Work", Some("💼")));
        assert!(
            matches!(&title.entities[..], [base::MessageEntity::MessageEntityCustomEmoji(e)] if e.document_id == 5)
        );
        assert!(f.groups && f.exclude_read && !f.contacts && !f.broadcasts && !f.title_noanimate);
        assert!(f.include_peers.is_empty() && f.color.is_none());
    }

    #[tokio::test]
    async fn a_title_mention_of_a_user_never_met_is_dropped() {
        use crate::raw::Serializable;
        use crate::types::User;

        let (client, _sent) = Client::scripted(vec![Ok(Vec::<i64>::new().to_bytes())]);
        let mut user = User::empty();
        user.id = 777;
        let mention = MessageEntity::new(MessageEntityType::TextMention, 0, 3).user(user);
        let bold = MessageEntity::new(MessageEntityType::Bold, 0, 3);
        let Ok(base::DialogFilter::DialogFilter(f)) =
            Folder::new("Ann").title_entities([mention, bold]).write(&client).await
        else {
            panic!("expected a plain folder");
        };
        let base::TextWithEntities::TextWithEntities(title) = &f.title;
        assert!(matches!(&title.entities[..], [base::MessageEntity::MessageEntityBold(_)]), "{:?}", title.entities);
    }
}
