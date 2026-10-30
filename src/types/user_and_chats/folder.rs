use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::MessageEntityType;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::MessageEntity;
use crate::utils::get_channel_id;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folder {
    pub id: i32,
    pub title: String,
    pub title_entities: Vec<MessageEntity>,
    pub no_title_animation: bool,
    pub emoji: Option<String>,
    pub color: Option<i32>,
    pub pinned_chats: Vec<i64>,
    pub included_chats: Vec<i64>,
    pub excluded_chats: Vec<i64>,
    pub contacts: bool,
    pub non_contacts: bool,
    pub groups: bool,
    pub channels: bool,
    pub bots: bool,
    pub exclude_muted: bool,
    pub exclude_read: bool,
    pub exclude_archived: bool,
    /// Shared through an invite link. Telegram ignores exclusions and chat type flags on these.
    pub is_shareable: bool,
}

impl Folder {
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
            entities.push(write_entity(client, entity).await?);
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

async fn write_entity(client: &Client, e: &MessageEntity) -> Result<base::MessageEntity> {
    use MessageEntityType as T;
    use base::MessageEntity as E;
    let (offset, length) = (e.offset, e.length);
    Ok(match e.kind {
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
            let user_id = client.resolve_user(&user.id.into()).await?;
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
    })
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
