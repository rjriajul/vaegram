use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Chat, ChatFullInfo, Community, PeerRef, User};
use crate::utils::get_peer;

impl Client {
    pub async fn get_chat(&self, chat_id: impl Into<PeerRef>) -> Result<Chat> {
        let chat_id = self.peer_id(chat_id.into()).await?;
        if chat_id > 0 {
            let id = self.resolve_user(&chat_id.into()).await?;
            let base::users::UserFull::UserFull(full) = self.invoke(&functions::users::GetFullUser { id }).await?;
            let types::users::UserFull { full_user: base::UserFull::UserFull(mut user), chats, users } = *full;
            let entities = Entities::new(self, users, chats);
            let mut chat = Chat::parse(&get_peer(chat_id), &entities, false);
            add_full_photo(&mut chat, user.profile_photo.as_ref());
            chat.bio = user.about.take().filter(|about| !about.is_empty());
            chat.community = Community::linked(chat_id, &entities);
            chat.full_info = Some(Box::new(ChatFullInfo::parse_user(*user, &entities)));
            return Ok(chat);
        }
        let (full_chat, entities) = self.full_chat(self.resolve_chat(&chat_id.into()).await?).await?;
        let mut chat = Chat::parse(&get_peer(chat_id), &entities, false);
        let invite = |link: &Option<base::ExportedChatInvite>| match link {
            Some(base::ExportedChatInvite::ChatInviteExported(i)) => Some(i.link.clone()),
            _ => None,
        };
        let (about, info) = match full_chat {
            base::ChatFull::ChatFull(c) => {
                add_full_photo(&mut chat, c.chat_photo.as_ref());
                chat.invite_link = invite(&c.exported_invite);
                if let base::ChatParticipants::ChatParticipants(p) = &c.participants {
                    chat.members_count = Some(p.participants.len() as i32);
                }
                let mut c = *c;
                (std::mem::take(&mut c.about), ChatFullInfo::parse_chat(c))
            }
            base::ChatFull::ChannelFull(c) => {
                add_full_photo(&mut chat, Some(&c.chat_photo));
                chat.invite_link = invite(&c.exported_invite);
                chat.members_count = c.participants_count.or(chat.members_count);
                chat.guard_bot = c.guard_bot_id.map(|id| User::parse(id, &entities));
                let mut c = *c;
                (std::mem::take(&mut c.about), ChatFullInfo::parse_channel(c, &entities))
            }
            base::ChatFull::CommunityFull(mut c) => {
                add_full_photo(&mut chat, Some(&c.chat_photo));
                (std::mem::take(&mut c.about), ChatFullInfo::parse_community(&c))
            }
        };
        chat.full_info = Some(Box::new(info));
        chat.description = (!about.is_empty()).then_some(about);
        chat.community = Community::linked(chat_id, &entities);
        Ok(chat)
    }
}

/// Adds the video and sticker of the full reply's photo to the chat's photo, when it is the same photo.
fn add_full_photo(chat: &mut Chat, full: Option<&base::Photo>) {
    if let Some(base::Photo::Photo(full)) = full {
        chat.photo = chat.photo.take().map(|photo| photo.with_full(full));
    }
}

#[cfg(test)]
mod tests {
    use super::add_full_photo;
    use crate::client::sent;
    use crate::dispatcher::Entities;
    use crate::enums::BusinessAwayMessageSchedule;
    use crate::raw::functions::{channels, contacts, messages, users};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::{Birthday, Chat, ChatFullInfo, ChatPermissions, ChatPhoto, ChatPrivileges, ChatReactions};
    use crate::utils::{get_channel_id, get_peer};
    use crate::{Client, Error};

    const USER: i64 = 7;
    const GROUP: i64 = -5;

    fn notify_settings() -> base::PeerNotifySettings {
        let settings = types::PeerNotifySettings::deserialize_bare(&mut &0u32.to_le_bytes()[..]).unwrap();
        base::PeerNotifySettings::PeerNotifySettings(settings.into())
    }

    fn raw_channel(id: i64, title: &str) -> base::Chat {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        title.to_owned().serialize(&mut buf);
        base::ChatPhoto::ChatPhotoEmpty.serialize(&mut buf);
        0i32.serialize(&mut buf);
        let channel =
            types::Channel { access_hash: Some(1), ..types::Channel::deserialize_bare(&mut &buf[..]).unwrap() };
        base::Chat::Channel(channel.into())
    }

    fn channel_full(id: i64) -> types::ChannelFull {
        let mut buf = Vec::new();
        for word in [0u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        "about".to_owned().serialize(&mut buf);
        for n in [0i32, 0, 0] {
            n.serialize(&mut buf);
        }
        base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 }).serialize(&mut buf);
        notify_settings().serialize(&mut buf);
        Vec::<i64>::new().serialize(&mut buf);
        0i32.serialize(&mut buf);
        types::ChannelFull::deserialize_bare(&mut &buf[..]).unwrap()
    }

    fn invite(link: &str) -> base::ExportedChatInvite {
        let invite = types::ChatInviteExported {
            revoked: false,
            permanent: true,
            request_needed: false,
            link: link.into(),
            admin_id: 1,
            date: 0,
            start_date: None,
            expire_date: None,
            usage_limit: None,
            usage: None,
            requested: None,
            subscription_expired: None,
            title: None,
            subscription_pricing: None,
        };
        base::ExportedChatInvite::ChatInviteExported(invite.into())
    }

    fn sticker_set(short_name: &str) -> base::StickerSet {
        let set = types::StickerSet {
            archived: false,
            official: false,
            masks: false,
            emojis: false,
            text_color: false,
            channel_emoji_status: false,
            creator: false,
            installed_date: None,
            id: 1,
            access_hash: 1,
            title: "set".into(),
            short_name: short_name.into(),
            thumbs: None,
            thumb_dc_id: None,
            thumb_version: None,
            thumb_document_id: None,
            count: 0,
            hash: 0,
        };
        base::StickerSet::StickerSet(set.into())
    }

    fn chat_full_reply(full_chat: base::ChatFull, chats: Vec<base::Chat>) -> Vec<u8> {
        let reply = types::messages::ChatFull { full_chat, chats, users: Vec::new() };
        base::messages::ChatFull::ChatFull(reply.into()).to_bytes()
    }

    fn known_channel(client: &Client, id: i64) -> i64 {
        let peer = types::InputPeerChannel { channel_id: id, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(id), base::InputPeer::InputPeerChannel(Box::new(peer)));
        get_channel_id(id)
    }

    #[tokio::test]
    async fn channel_full_info_is_filled_from_the_one_reply() {
        let full = types::ChannelFull {
            participants_count: Some(31),
            linked_chat_id: Some(11),
            default_send_as: Some(get_peer(get_channel_id(11))),
            stickerset: Some(sticker_set("cats")),
            emojiset: Some(sticker_set("cat_emoji")),
            slowmode_seconds: Some(30),
            exported_invite: Some(invite("https://t.me/+abc")),
            pinned_msg_id: Some(77),
            migrated_from_chat_id: Some(5),
            migrated_from_max_id: Some(100),
            hidden_prehistory: true,
            can_set_stickers: true,
            antispam: true,
            theme_emoticon: Some("🐱".into()),
            ..channel_full(9)
        };
        let reply = chat_full_reply(
            base::ChatFull::ChannelFull(full.into()),
            vec![raw_channel(9, "main"), raw_channel(11, "linked")],
        );
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let chat = client.get_chat(known_channel(&client, 9)).await.unwrap();
        rx.try_recv().expect("the full chat request");
        assert!(rx.try_recv().is_err(), "get_chat sent a second request");

        let info = chat.full_info.unwrap();
        assert_eq!((chat.description.as_deref(), chat.members_count), (Some("about"), Some(31)));
        assert_eq!(info.migrated_from_chat_id, Some(-5));
        assert_eq!(info.migrated_from_max_message_id, Some(100));
        let linked = info.linked_chat.as_ref().unwrap();
        assert_eq!((linked.id, linked.title.as_deref()), (get_channel_id(11), Some("linked")));
        assert_eq!(info.send_as_chat.as_ref().map(|c| c.id), Some(get_channel_id(11)));
        assert_eq!(info.sticker_set_name.as_deref(), Some("cats"));
        assert_eq!(info.custom_emoji_sticker_set_name.as_deref(), Some("cat_emoji"));
        assert_eq!((info.slow_mode_delay, info.pinned_message_id), (Some(30), Some(77)));
        assert_eq!(info.invite_link.as_deref(), Some("https://t.me/+abc"));
        assert_eq!(info.theme.as_deref(), Some("🐱"));
        let flags = (info.has_visible_history, info.can_set_sticker_set, info.has_aggressive_anti_spam_enabled);
        assert_eq!(flags, (false, true, true));
        assert_eq!((info.available_reactions, info.can_view_participants), (None, false));
    }

    #[tokio::test]
    async fn group_full_info_reads_invite_and_reactions() {
        let participants = types::ChatParticipantsForbidden { chat_id: 5, self_participant: None };
        let full = types::ChatFull {
            can_set_username: true,
            has_scheduled: true,
            translations_disabled: false,
            has_welcome_messages: false,
            id: 5,
            about: String::new(),
            participants: base::ChatParticipants::ChatParticipantsForbidden(participants.into()),
            chat_photo: None,
            notify_settings: notify_settings(),
            exported_invite: Some(invite("https://t.me/+group")),
            bot_info: None,
            pinned_msg_id: Some(3),
            folder_id: Some(1),
            call: None,
            ttl_period: Some(86400),
            groupcall_default_join_as: None,
            theme_emoticon: None,
            requests_pending: Some(2),
            recent_requesters: Some(vec![8, 9]),
            available_reactions: Some(base::ChatReactions::ChatReactionsNone),
            reactions_limit: Some(11),
        };
        let reply = chat_full_reply(base::ChatFull::ChatFull(full.into()), Vec::new());
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let chat = client.get_chat(GROUP).await.unwrap();
        rx.try_recv().expect("the full chat request");
        assert!(rx.try_recv().is_err());

        let info = chat.full_info.unwrap();
        assert_eq!(chat.description, None);
        assert_eq!(info.invite_link.as_deref(), Some("https://t.me/+group"));
        // No reactions allowed is a value, not a missing field.
        assert_eq!(info.available_reactions, Some(ChatReactions::default()));
        assert_eq!((info.reactions_limit, info.join_requests_count), (Some(11), Some(2)));
        assert_eq!(
            (info.pinned_message_id, info.folder_id, info.message_auto_delete_time),
            (Some(3), Some(1), Some(86400))
        );
        assert_eq!((info.can_set_username, info.can_schedule_messages, info.linked_chat), (true, true, None));
    }

    #[tokio::test]
    async fn a_full_reply_without_a_count_keeps_the_constructor_count() {
        let base::Chat::Channel(mut channel) = raw_channel(9, "main") else { unreachable!() };
        channel.participants_count = Some(40);
        let full = base::ChatFull::ChannelFull(channel_full(9).into());
        let reply = chat_full_reply(full, vec![base::Chat::Channel(channel)]);
        let (client, _rx) = Client::scripted(vec![Ok(reply)]);
        let chat = client.get_chat(known_channel(&client, 9)).await.unwrap();
        assert_eq!(chat.members_count, Some(40));
    }

    fn user_full(id: i64) -> types::UserFull {
        let mut buf = Vec::new();
        for word in [0u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        types::PeerSettings::CONSTRUCTOR_ID.serialize(&mut buf);
        0u32.serialize(&mut buf);
        notify_settings().serialize(&mut buf);
        3i32.serialize(&mut buf);
        types::UserFull::deserialize_bare(&mut &buf[..]).unwrap()
    }

    fn raw_user(id: i64, access_hash: i64) -> base::User {
        let mut buf = Vec::new();
        for word in [1u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        access_hash.serialize(&mut buf);
        base::User::User(types::User::deserialize_bare(&mut &buf[..]).unwrap().into())
    }

    #[tokio::test]
    async fn full_photo_adds_animation() {
        let video = types::VideoSize { r#type: "u".into(), w: 800, h: 800, size: 20, video_start_ts: Some(0.5) };
        let full_photo = types::Photo {
            has_stickers: false,
            id: 55,
            access_hash: 1,
            file_reference: Vec::new(),
            date: 0,
            sizes: Vec::new(),
            video_sizes: Some(vec![base::VideoSize::VideoSize(video.into())]),
            dc_id: 4,
        };
        let profile =
            types::UserProfilePhoto { has_video: true, personal: false, photo_id: 55, stripped_thumb: None, dc_id: 4 };
        let base::User::User(user) = raw_user(USER, 77) else { unreachable!() };
        let user = types::User { photo: Some(base::UserProfilePhoto::UserProfilePhoto(profile.into())), ..*user };
        let full = types::UserFull { profile_photo: Some(base::Photo::Photo(full_photo.into())), ..user_full(USER) };
        let reply = types::users::UserFull {
            full_user: base::UserFull::UserFull(full.into()),
            chats: Vec::new(),
            users: vec![base::User::User(user.into())],
        };
        let reply = base::users::UserFull::UserFull(reply.into()).to_bytes();
        let (client, _rx) = Client::scripted(vec![Ok(reply)]);
        let peer = types::InputPeerUser { user_id: USER, access_hash: 77 };
        client.0.peers.lock().unwrap().insert(USER, base::InputPeer::InputPeerUser(Box::new(peer)));
        let chat = client.get_chat(USER).await.unwrap();
        let animation = chat.photo.unwrap().animation.expect("an animation");
        assert_eq!((animation.length, animation.main_frame_timestamp), (800, Some(0.5)));
    }

    #[test]
    fn a_missing_or_empty_full_photo_keeps_the_chat_photo() {
        let photo = ChatPhoto::new(USER, 77, 55, 4, true, false);
        let mut chat = Chat::parse(&get_peer(USER), &Entities::default(), false);
        chat.photo = Some(photo.clone());
        add_full_photo(&mut chat, None);
        assert_eq!(chat.photo, Some(photo.clone()));
        let empty = base::Photo::PhotoEmpty(types::PhotoEmpty { id: 55 });
        add_full_photo(&mut chat, Some(&empty));
        assert_eq!(chat.photo, Some(photo));
    }

    fn recipients() -> base::BusinessRecipients {
        let r = types::BusinessRecipients {
            existing_chats: false,
            new_chats: true,
            contacts: false,
            non_contacts: false,
            exclude_selected: false,
            users: None,
        };
        base::BusinessRecipients::BusinessRecipients(r.into())
    }

    #[tokio::test]
    async fn user_full_info_reads_business_birthday_and_rating() {
        let photo = types::Photo {
            has_stickers: false,
            id: 55,
            access_hash: 1,
            file_reference: Vec::new(),
            date: 0,
            sizes: Vec::new(),
            video_sizes: None,
            dc_id: 4,
        };
        let rating = |stars, next_level_stars| {
            let r = types::StarsRating { level: 2, current_level_stars: 10, stars, next_level_stars };
            base::StarsRating::StarsRating(r.into())
        };
        let intro = types::BusinessIntro { title: "Hi".into(), description: "Ask away".into(), sticker: None };
        let location = types::BusinessLocation { geo_point: None, address: "1 Cat Street".into() };
        let hours =
            types::BusinessWorkHours { open_now: true, timezone_id: "Asia/Tokyo".into(), weekly_open: Vec::new() };
        let greeting = types::BusinessGreetingMessage { shortcut_id: 1, recipients: recipients(), no_activity_days: 7 };
        let away = types::BusinessAwayMessage {
            offline_only: true,
            shortcut_id: 2,
            schedule: base::BusinessAwayMessageSchedule::BusinessAwayMessageScheduleAlways,
            recipients: recipients(),
        };
        let full = types::UserFull {
            about: Some("bio".into()),
            voice_messages_forbidden: true,
            read_dates_private: true,
            phone_calls_available: true,
            birthday: Some(base::Birthday::Birthday(types::Birthday { day: 1, month: 2, year: None }.into())),
            personal_channel_id: Some(11),
            personal_channel_message: Some(5),
            fallback_photo: Some(base::Photo::Photo(photo.into())),
            business_intro: Some(base::BusinessIntro::BusinessIntro(intro.into())),
            business_location: Some(base::BusinessLocation::BusinessLocation(location.into())),
            business_work_hours: Some(base::BusinessWorkHours::BusinessWorkHours(hours.into())),
            business_greeting_message: Some(base::BusinessGreetingMessage::BusinessGreetingMessage(greeting.into())),
            business_away_message: Some(base::BusinessAwayMessage::BusinessAwayMessage(away.into())),
            stars_rating: Some(rating(15, Some(20))),
            stars_my_pending_rating: Some(rating(18, None)),
            stars_my_pending_rating_date: Some(1000),
            theme: Some(base::ChatTheme::ChatTheme(types::ChatTheme { emoticon: "🐱".into() }.into())),
            ..user_full(USER)
        };
        let reply = types::users::UserFull {
            full_user: base::UserFull::UserFull(full.into()),
            chats: vec![raw_channel(11, "diary")],
            users: vec![raw_user(USER, 77)],
        };
        let reply = base::users::UserFull::UserFull(reply.into()).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let user = types::InputPeerUser { user_id: USER, access_hash: 77 };
        client.0.peers.lock().unwrap().insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
        let chat = client.get_chat(USER).await.unwrap();
        rx.try_recv().expect("the full user request");
        assert!(rx.try_recv().is_err(), "get_chat sent a second request");

        let info = chat.full_info.unwrap();
        assert_eq!(chat.bio.as_deref(), Some("bio"));
        assert_eq!(info.birthday, Some(Birthday { day: 1, month: 2, year: None }));
        let channel = info.personal_channel.as_ref().unwrap();
        assert_eq!((channel.id, channel.title.as_deref()), (get_channel_id(11), Some("diary")));
        assert_eq!((info.personal_channel_message_id, info.common_chats_count), (Some(5), Some(3)));
        assert_eq!(info.public_photo, Some(ChatPhoto::new(USER, 77, 55, 4, false, false)));
        assert_eq!(info.personal_photo, None);
        let flags = (info.can_send_voice_messages, info.is_read_dates_available, info.is_phone_calls_available);
        assert_eq!(flags, (false, false, true));
        assert_eq!(info.business_intro.map(|i| i.text), Some("Ask away".into()));
        assert_eq!(info.business_location.map(|l| l.address), Some("1 Cat Street".into()));
        assert_eq!(info.business_opening_hours.map(|h| h.is_open_now), Some(true));
        assert_eq!(
            info.business_greeting_message.map(|g| (g.no_activity_days, g.recipients.new_chats)),
            Some((7, true))
        );
        let away = info.business_away_message.unwrap();
        assert_eq!((away.schedule, away.offline_only), (BusinessAwayMessageSchedule::Always, true));
        assert_eq!(info.rating.map(|r| (r.rating, r.is_maximum_level_reached)), Some((15, false)));
        assert_eq!(info.pending_rating.map(|r| r.is_maximum_level_reached), Some(true));
        assert_eq!((info.pending_rating_date, info.theme.as_deref()), (Some(1000), Some("🐱")));

        // The raw flags say what is forbidden; unset, the friendly fields allow it.
        let open = ChatFullInfo::parse_user(user_full(USER), &Entities::default());
        assert_eq!((open.can_send_voice_messages, open.is_read_dates_available, open.public_photo), (true, true, None));
    }

    #[tokio::test]
    async fn dispatches_on_chat_id_kind() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        {
            let mut peers = client.0.peers.lock().unwrap();
            let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
            peers.insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
            let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
            peers.insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));
        }
        let rx = &mut rx;
        let admin = ChatPrivileges { can_pin_messages: true, ..Default::default() };
        let muted = ChatPermissions::default();

        assert_eq!(sent(rx, client.get_chat(USER)).await, users::GetFullUser::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat(-5)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat(channel)).await, channels::GetFullChannel::CONSTRUCTOR_ID);

        assert_eq!(sent(rx, client.get_chat_member(GROUP, USER)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_member(channel, USER)).await, channels::GetParticipant::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members(GROUP)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members(channel)).await, channels::GetParticipants::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_administrators(GROUP)).await, messages::GetFullChat::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_administrators(channel)).await, channels::GetParticipants::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members_count(GROUP)).await, messages::GetChats::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_chat_members_count(channel)).await, channels::GetFullChannel::CONSTRUCTOR_ID);

        assert_eq!(sent(rx, client.ban_chat_member(GROUP, USER)).await, messages::DeleteChatUser::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.ban_chat_member(channel, USER)).await, channels::EditBanned::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.unban_chat_member(channel, USER)).await, channels::EditBanned::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, client.restrict_chat_member(channel, USER, muted)).await,
            channels::EditBanned::CONSTRUCTOR_ID
        );
        assert_eq!(
            sent(rx, client.promote_chat_member(GROUP, USER, admin)).await,
            messages::EditChatAdmin::CONSTRUCTOR_ID
        );
        assert_eq!(
            sent(rx, client.promote_chat_member(channel, USER, admin)).await,
            channels::EditAdmin::CONSTRUCTOR_ID
        );
        assert_eq!(sent(rx, client.leave_chat(GROUP)).await, messages::DeleteChatUser::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.leave_chat(channel)).await, channels::LeaveChannel::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_chat_title(GROUP, "t")).await, messages::EditChatTitle::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_chat_title(channel, "t")).await, channels::EditTitle::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.set_chat_description(channel, "d")).await, messages::EditChatAbout::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.join_chat("t.me/+hash")).await, messages::ImportChatInvite::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.join_chat("@group")).await, contacts::ResolveUsername::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_users([USER, 8, 9])).await, users::GetUsers::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, client.get_users(["me"])).await, users::GetUsers::CONSTRUCTOR_ID);

        for result in [
            client.leave_chat(USER).await,
            client.set_chat_title(USER, "t").await,
            client.restrict_chat_member(GROUP, USER, muted).await,
            client.unban_chat_member(GROUP, USER).await,
        ] {
            assert!(matches!(result, Err(Error::InvalidArgument(_))));
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn rejects_a_user_id_without_a_request() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: USER, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(USER, base::InputPeer::InputPeerUser(Box::new(user)));
        assert!(matches!(client.leave_chat(USER).await, Err(Error::InvalidArgument(_))));
        let muted = ChatPermissions::default();
        assert!(matches!(client.restrict_chat_member(GROUP, USER, muted).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn server_peer_id_invalid_stays_an_rpc_error() {
        let refused = crate::errors::RpcError::new(400, "PEER_ID_INVALID");
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(refused))]);
        let result = client.leave_chat(GROUP).await;
        assert!(matches!(result, Err(Error::Rpc(e)) if e.name == "PEER_ID_INVALID"));
    }
}
