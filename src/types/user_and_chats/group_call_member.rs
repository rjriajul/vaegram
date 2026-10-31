use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::Chat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupCallMember {
    /// A user, or a chat speaking as itself.
    pub chat: Chat,
    pub date: i32,
    pub active_date: Option<i32>,
    /// Hundredths of a percent: 10000 is 100%.
    pub volume: Option<i32>,
    pub about: Option<String>,
    pub paid_star_count: Option<i64>,
    pub is_muted: bool,
    pub is_left: bool,
    pub can_self_unmute: bool,
    pub is_just_joined: bool,
    pub is_muted_by_you: bool,
    pub is_volume_by_admin: bool,
    pub is_self: bool,
    pub is_video_joined: bool,
    pub is_hand_raised: bool,
    pub is_video_enabled: bool,
    pub is_screen_sharing_enabled: bool,
}

impl GroupCallMember {
    pub(crate) fn parse(member: base::GroupCallParticipant, entities: &Entities) -> GroupCallMember {
        let base::GroupCallParticipant::GroupCallParticipant(m) = member;
        GroupCallMember {
            chat: Chat::parse(&m.peer, entities, false),
            date: m.date,
            active_date: m.active_date,
            volume: m.volume,
            about: m.about,
            paid_star_count: m.paid_stars_total,
            is_muted: m.muted,
            is_left: m.left,
            can_self_unmute: m.can_self_unmute,
            is_just_joined: m.just_joined,
            is_muted_by_you: m.muted_by_you,
            is_volume_by_admin: m.volume_by_admin,
            is_self: m.is_self,
            is_video_joined: m.video_joined,
            is_hand_raised: m.raise_hand_rating.is_some(),
            is_video_enabled: m.video.is_some(),
            is_screen_sharing_enabled: m.presentation.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GroupCallMember;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{Serializable, base, types};
    use crate::utils::get_channel_id;

    fn participant(peer: base::Peer) -> types::GroupCallParticipant {
        types::GroupCallParticipant {
            muted: false,
            left: false,
            can_self_unmute: false,
            just_joined: false,
            versioned: false,
            min: false,
            muted_by_you: false,
            volume_by_admin: false,
            is_self: false,
            video_joined: false,
            peer,
            date: 10,
            active_date: None,
            source: 1,
            volume: None,
            about: None,
            raise_hand_rating: None,
            video: None,
            presentation: None,
            paid_stars_total: None,
        }
    }

    fn named_user(id: i64, name: &str) -> base::User {
        let mut buf = Vec::new();
        // Only access_hash and first_name are set.
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        name.to_owned().serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn user_peer(user_id: i64) -> base::Peer {
        base::Peer::PeerUser(types::PeerUser { user_id })
    }

    #[test]
    fn maps_every_flag_and_field() {
        let video = types::GroupCallParticipantVideo {
            paused: false,
            endpoint: "e".into(),
            source_groups: Vec::new(),
            audio_source: None,
        };
        let video = base::GroupCallParticipantVideo::GroupCallParticipantVideo(Box::new(video));
        let raw = types::GroupCallParticipant {
            muted: true,
            left: true,
            can_self_unmute: true,
            just_joined: true,
            muted_by_you: true,
            volume_by_admin: true,
            is_self: true,
            video_joined: true,
            active_date: Some(11),
            volume: Some(5000),
            about: Some("bio".into()),
            raise_hand_rating: Some(3),
            video: Some(video.clone()),
            presentation: Some(video),
            paid_stars_total: Some(25),
            ..participant(user_peer(7))
        };
        let entities = Entities::new(&Client::offline(), vec![named_user(7, "Ann")], Vec::new());
        let m = GroupCallMember::parse(base::GroupCallParticipant::GroupCallParticipant(Box::new(raw)), &entities);
        assert_eq!((m.chat.id, m.chat.first_name.as_deref(), m.date), (7, Some("Ann"), 10));
        assert_eq!(
            (m.active_date, m.volume, m.about.as_deref(), m.paid_star_count),
            (Some(11), Some(5000), Some("bio"), Some(25))
        );
        assert!(m.is_muted && m.is_left && m.can_self_unmute && m.is_just_joined);
        assert!(m.is_muted_by_you && m.is_volume_by_admin && m.is_self && m.is_video_joined);
        assert!(m.is_hand_raised && m.is_video_enabled && m.is_screen_sharing_enabled);

        let quiet = GroupCallMember::parse(
            base::GroupCallParticipant::GroupCallParticipant(Box::new(participant(user_peer(7)))),
            &entities,
        );
        assert!(
            !quiet.is_muted && !quiet.is_hand_raised && !quiet.is_video_enabled && !quiet.is_screen_sharing_enabled
        );
    }

    #[test]
    fn chats_and_unknown_users_still_parse() {
        let channel = base::Peer::PeerChannel(types::PeerChannel { channel_id: 9 });
        let entities = Entities::default();
        let m = GroupCallMember::parse(
            base::GroupCallParticipant::GroupCallParticipant(Box::new(participant(channel))),
            &entities,
        );
        assert_eq!(m.chat.id, get_channel_id(9));

        // A member missing from `users` keeps its id, with no name.
        let m = GroupCallMember::parse(
            base::GroupCallParticipant::GroupCallParticipant(Box::new(participant(user_peer(8)))),
            &entities,
        );
        assert_eq!((m.chat.id, m.chat.first_name), (8, None));
    }
}
