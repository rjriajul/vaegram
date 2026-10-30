use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{ChatMember, ChatPermissions, Message, Photo, User};
use crate::utils::get_channel_id;

#[derive(Debug, Clone)]
pub struct ChatEvent {
    pub id: i64,
    pub date: i32,
    pub user: User,
    pub action: ChatEventAction,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ChatEventAction {
    TitleChanged { old: String, new: String },
    DescriptionChanged { old: String, new: String },
    UsernameChanged { old: String, new: String },
    PhotoChanged { old: Option<Photo>, new: Option<Photo> },
    LinkedChatChanged { old: Option<i64>, new: Option<i64> },
    SlowModeChanged { old: i32, new: i32 },
    HistoryTtlChanged { old: i32, new: i32 },
    ChatPermissionsChanged { old: ChatPermissions, new: ChatPermissions },
    InvitesEnabled(bool),
    SignaturesEnabled(bool),
    HistoryHidden(bool),
    ContentProtected(bool),
    ForumTopicsEnabled(bool),
    MessagePinned(Message),
    MessageUnpinned(Message),
    MessageEdited { old: Box<Message>, new: Box<Message> },
    MessageDeleted(Message),
    MessageSent(Message),
    PollStopped(Message),
    MemberJoined,
    MemberLeft,
    MemberJoinedByLink { invite_link: String, via_chatlist: bool },
    MemberJoinedByRequest { invite_link: String, approved_by: User },
    MemberInvited(ChatMember),
    MemberPermissionsChanged { old: ChatMember, new: ChatMember },
    AdministratorPrivilegesChanged { old: ChatMember, new: ChatMember },
    InviteLinkEdited { old: String, new: String },
    InviteLinkRevoked(String),
    InviteLinkDeleted(String),
    VideoChatStarted,
    VideoChatEnded,
    Unknown,
}

impl ChatEvent {
    pub(crate) fn parse(client: &Client, raw: base::ChannelAdminLogEvent, entities: &Entities) -> ChatEvent {
        let base::ChannelAdminLogEvent::ChannelAdminLogEvent(raw) = raw;
        let action = ChatEventAction::parse(client, &raw.action, entities).unwrap_or(ChatEventAction::Unknown);
        ChatEvent { id: raw.id, date: raw.date, user: User::parse(raw.user_id, entities), action }
    }
}

impl ChatEventAction {
    fn parse(client: &Client, action: &base::ChannelAdminLogEventAction, entities: &Entities) -> Option<Self> {
        use base::ChannelAdminLogEventAction as A;
        let message = |m: &base::Message| Message::from_raw(client, m.clone(), entities);
        let member = |p: &base::ChannelParticipant| ChatMember::parse_channel(p, entities);
        let channel = |id: i64| (id != 0).then(|| get_channel_id(id));
        Some(match action {
            A::ChannelAdminLogEventActionChangeTitle(a) => {
                Self::TitleChanged { old: a.prev_value.clone(), new: a.new_value.clone() }
            }
            A::ChannelAdminLogEventActionChangeAbout(a) => {
                Self::DescriptionChanged { old: a.prev_value.clone(), new: a.new_value.clone() }
            }
            A::ChannelAdminLogEventActionChangeUsername(a) => {
                Self::UsernameChanged { old: a.prev_value.clone(), new: a.new_value.clone() }
            }
            A::ChannelAdminLogEventActionChangePhoto(a) => {
                Self::PhotoChanged { old: Photo::parse(&a.prev_photo), new: Photo::parse(&a.new_photo) }
            }
            A::ChannelAdminLogEventActionChangeLinkedChat(a) => {
                Self::LinkedChatChanged { old: channel(a.prev_value), new: channel(a.new_value) }
            }
            A::ChannelAdminLogEventActionToggleSlowMode(a) => {
                Self::SlowModeChanged { old: a.prev_value, new: a.new_value }
            }
            A::ChannelAdminLogEventActionChangeHistoryTTL(a) => {
                Self::HistoryTtlChanged { old: a.prev_value, new: a.new_value }
            }
            A::ChannelAdminLogEventActionDefaultBannedRights(a) => Self::ChatPermissionsChanged {
                old: ChatPermissions::parse(&a.prev_banned_rights),
                new: ChatPermissions::parse(&a.new_banned_rights),
            },
            A::ChannelAdminLogEventActionToggleInvites(a) => Self::InvitesEnabled(a.new_value),
            A::ChannelAdminLogEventActionToggleSignatures(a) => Self::SignaturesEnabled(a.new_value),
            A::ChannelAdminLogEventActionTogglePreHistoryHidden(a) => Self::HistoryHidden(a.new_value),
            A::ChannelAdminLogEventActionToggleNoForwards(a) => Self::ContentProtected(a.new_value),
            A::ChannelAdminLogEventActionToggleForum(a) => Self::ForumTopicsEnabled(a.new_value),
            A::ChannelAdminLogEventActionUpdatePinned(a) => match &a.message {
                base::Message::Message(m) if m.pinned => Self::MessagePinned(message(&a.message)?),
                _ => Self::MessageUnpinned(message(&a.message)?),
            },
            A::ChannelAdminLogEventActionEditMessage(a) => Self::MessageEdited {
                old: Box::new(message(&a.prev_message)?),
                new: Box::new(message(&a.new_message)?),
            },
            A::ChannelAdminLogEventActionDeleteMessage(a) => Self::MessageDeleted(message(&a.message)?),
            A::ChannelAdminLogEventActionSendMessage(a) => Self::MessageSent(message(&a.message)?),
            A::ChannelAdminLogEventActionStopPoll(a) => Self::PollStopped(message(&a.message)?),
            A::ChannelAdminLogEventActionParticipantJoin => Self::MemberJoined,
            A::ChannelAdminLogEventActionParticipantLeave => Self::MemberLeft,
            A::ChannelAdminLogEventActionParticipantJoinByInvite(a) => {
                Self::MemberJoinedByLink { invite_link: invite_link(&a.invite), via_chatlist: a.via_chatlist }
            }
            A::ChannelAdminLogEventActionParticipantJoinByRequest(a) => Self::MemberJoinedByRequest {
                invite_link: invite_link(&a.invite),
                approved_by: User::parse(a.approved_by, entities),
            },
            A::ChannelAdminLogEventActionParticipantInvite(a) => Self::MemberInvited(member(&a.participant)),
            A::ChannelAdminLogEventActionParticipantToggleBan(a) => {
                Self::MemberPermissionsChanged { old: member(&a.prev_participant), new: member(&a.new_participant) }
            }
            A::ChannelAdminLogEventActionParticipantToggleAdmin(a) => Self::AdministratorPrivilegesChanged {
                old: member(&a.prev_participant),
                new: member(&a.new_participant),
            },
            A::ChannelAdminLogEventActionExportedInviteEdit(a) => {
                Self::InviteLinkEdited { old: invite_link(&a.prev_invite), new: invite_link(&a.new_invite) }
            }
            A::ChannelAdminLogEventActionExportedInviteRevoke(a) => Self::InviteLinkRevoked(invite_link(&a.invite)),
            A::ChannelAdminLogEventActionExportedInviteDelete(a) => Self::InviteLinkDeleted(invite_link(&a.invite)),
            A::ChannelAdminLogEventActionStartGroupCall(_) => Self::VideoChatStarted,
            A::ChannelAdminLogEventActionDiscardGroupCall(_) => Self::VideoChatEnded,
            _ => return None,
        })
    }
}

// The public join-request link of a discussion group has no URL, so it reads as an empty string.
fn invite_link(invite: &base::ExportedChatInvite) -> String {
    match invite {
        base::ExportedChatInvite::ChatInviteExported(i) => i.link.clone(),
        base::ExportedChatInvite::ChatInvitePublicJoinRequests => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatEvent, ChatEventAction};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    fn event(action: base::ChannelAdminLogEventAction) -> ChatEventAction {
        let raw = types::ChannelAdminLogEvent { id: 1, date: 2, user_id: 3, action };
        let raw = base::ChannelAdminLogEvent::ChannelAdminLogEvent(Box::new(raw));
        let event = ChatEvent::parse(&Client::offline(), raw, &Entities::default());
        assert_eq!((event.id, event.date, event.user.id), (1, 2, 3));
        event.action
    }

    #[test]
    fn maps_common_actions_and_keeps_the_rest_raw() {
        use base::ChannelAdminLogEventAction as A;
        let title = types::ChannelAdminLogEventActionChangeTitle { prev_value: "a".into(), new_value: "b".into() };
        let slow = types::ChannelAdminLogEventActionToggleSlowMode { prev_value: 0, new_value: 30 };
        let linked = types::ChannelAdminLogEventActionChangeLinkedChat { prev_value: 0, new_value: 5 };
        let peer = base::Peer::PeerUser(types::PeerUser { user_id: 9 });
        let left = base::ChannelParticipant::ChannelParticipantLeft(Box::new(types::ChannelParticipantLeft { peer }));
        let ban = types::ChannelAdminLogEventActionParticipantToggleBan {
            prev_participant: left.clone(),
            new_participant: left,
        };
        let anti_spam = types::ChannelAdminLogEventActionToggleAntiSpam { new_value: true };
        let anti_spam = A::ChannelAdminLogEventActionToggleAntiSpam(anti_spam);
        let empty = base::Message::MessageEmpty(Box::new(types::MessageEmpty { id: 4, peer_id: None }));
        let deleted = types::ChannelAdminLogEventActionDeleteMessage { message: empty };
        let deleted = A::ChannelAdminLogEventActionDeleteMessage(Box::new(deleted));

        assert!(matches!(
            event(A::ChannelAdminLogEventActionChangeTitle(Box::new(title))),
            ChatEventAction::TitleChanged { old, new } if (old.as_str(), new.as_str()) == ("a", "b")
        ));
        assert!(matches!(
            event(A::ChannelAdminLogEventActionToggleSlowMode(slow)),
            ChatEventAction::SlowModeChanged { old: 0, new: 30 }
        ));
        assert!(matches!(
            event(A::ChannelAdminLogEventActionChangeLinkedChat(Box::new(linked))),
            ChatEventAction::LinkedChatChanged { old: None, new: Some(-1_000_000_000_005) }
        ));
        assert!(matches!(
            event(A::ChannelAdminLogEventActionParticipantToggleBan(Box::new(ban))),
            ChatEventAction::MemberPermissionsChanged { new, .. } if new.user.id == 9
        ));
        assert!(matches!(event(A::ChannelAdminLogEventActionParticipantJoin), ChatEventAction::MemberJoined));
        assert!(matches!(event(anti_spam), ChatEventAction::Unknown));
        assert!(matches!(event(deleted), ChatEventAction::Unknown));
    }
}
