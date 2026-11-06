use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{ChatMember, ChatPermissions, Message, Photo, User};
use crate::utils::get_channel_id;

/// An entry in a chat's admin log.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ChatEvent {
    /// Id of the log entry, unique within the chat's log.
    pub id: i64,
    /// Unix time the action happened.
    pub date: i32,
    /// The user who did the action.
    pub user: User,
    /// The action that was done.
    pub action: ChatEventAction,
}

/// What an admin log entry records.
///
/// Actions vaegram doesn't map, and entries whose message fails to parse, come as [`ChatEventAction::Unknown`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ChatEventAction {
    /// The chat title changed.
    #[non_exhaustive]
    TitleChanged {
        /// The title before.
        old: String,
        /// The title after.
        new: String,
    },
    /// The chat description changed.
    #[non_exhaustive]
    DescriptionChanged {
        /// The description before; empty if there was none.
        old: String,
        /// The description after; empty if it was removed.
        new: String,
    },
    /// The chat's public username changed.
    #[non_exhaustive]
    UsernameChanged {
        /// The username before, without `@`; empty if the chat had none.
        old: String,
        /// The username after, without `@`; empty if it was removed.
        new: String,
    },
    /// The chat photo changed. `None` means no photo.
    #[non_exhaustive]
    PhotoChanged {
        /// The photo before.
        old: Option<Photo>,
        /// The photo after.
        new: Option<Photo>,
    },
    /// The linked discussion chat changed. `None` means no linked chat.
    #[non_exhaustive]
    LinkedChatChanged {
        /// Bot-API id (`-100…`) of the chat linked before.
        old: Option<i64>,
        /// Bot-API id (`-100…`) of the chat linked after.
        new: Option<i64>,
    },
    /// The slow mode delay changed, in seconds.
    #[non_exhaustive]
    SlowModeChanged {
        /// The delay before; 0 means slow mode was off.
        old: i32,
        /// The delay after; 0 means slow mode is off.
        new: i32,
    },
    /// The auto-delete timer for messages changed, in seconds.
    #[non_exhaustive]
    HistoryTtlChanged {
        /// The timer before; 0 means it was off.
        old: i32,
        /// The timer after; 0 means it is off.
        new: i32,
    },
    /// The default permissions of members changed.
    #[non_exhaustive]
    ChatPermissionsChanged {
        /// The permissions before.
        old: ChatPermissions,
        /// The permissions after.
        new: ChatPermissions,
    },
    /// Inviting users was turned on or off. Holds the new setting.
    InvitesEnabled(bool),
    /// Author signatures on posts were turned on or off. Holds the new setting.
    SignaturesEnabled(bool),
    /// Hiding old history from new members was turned on or off. Holds the new setting.
    HistoryHidden(bool),
    /// Protection against forwarding and saving was turned on or off. Holds the new setting.
    ContentProtected(bool),
    /// Forum topics were turned on or off. Holds the new setting.
    ForumTopicsEnabled(bool),
    /// A message was pinned.
    MessagePinned(Message),
    /// A message was unpinned.
    MessageUnpinned(Message),
    /// A message was edited.
    #[non_exhaustive]
    MessageEdited {
        /// The message before the edit.
        old: Box<Message>,
        /// The message after the edit.
        new: Box<Message>,
    },
    /// A message was deleted. Holds the message as it was.
    MessageDeleted(Message),
    /// A message was sent.
    MessageSent(Message),
    /// A poll was stopped. Holds the poll message.
    PollStopped(Message),
    /// A user joined the chat.
    MemberJoined,
    /// A user left the chat.
    MemberLeft,
    /// A user joined through an invite link. The link is empty for a public join-request link.
    #[non_exhaustive]
    MemberJoinedByLink {
        /// The link used; empty for a public join-request link.
        invite_link: String,
        /// The link was opened as part of a chat folder link.
        via_chatlist: bool,
    },
    /// A user joined after a join request was approved.
    #[non_exhaustive]
    MemberJoinedByRequest {
        /// The link the request came through; empty for a public join-request link.
        invite_link: String,
        /// The admin who approved the request.
        approved_by: User,
    },
    /// A user was added to the chat.
    MemberInvited(ChatMember),
    /// A member was banned, unbanned or restricted, or their restrictions changed.
    #[non_exhaustive]
    MemberPermissionsChanged {
        /// The member before the change.
        old: ChatMember,
        /// The member after the change.
        new: ChatMember,
    },
    /// A member was promoted, demoted, or their administrator rights changed.
    #[non_exhaustive]
    AdministratorPrivilegesChanged {
        /// The member before the change.
        old: ChatMember,
        /// The member after the change.
        new: ChatMember,
    },
    /// An invite link was edited.
    #[non_exhaustive]
    InviteLinkEdited {
        /// The link before the edit.
        old: String,
        /// The link after the edit.
        new: String,
    },
    /// An invite link was revoked.
    InviteLinkRevoked(String),
    /// An invite link was deleted.
    InviteLinkDeleted(String),
    /// A video chat was started.
    VideoChatStarted,
    /// A video chat was ended.
    VideoChatEnded,
    /// An action vaegram doesn't map, or one whose message failed to parse.
    #[non_exhaustive]
    Unknown {
        /// TL constructor id of the admin log action, for logging.
        constructor_id: u32,
    },
}

impl ChatEvent {
    pub(crate) fn parse(client: &Client, raw: base::ChannelAdminLogEvent, entities: &Entities) -> ChatEvent {
        let base::ChannelAdminLogEvent::ChannelAdminLogEvent(raw) = raw;
        let action = ChatEventAction::parse(client, &raw.action, entities)
            .unwrap_or(ChatEventAction::Unknown { constructor_id: raw.action.constructor_id() });
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
        assert!(matches!(event(anti_spam), ChatEventAction::Unknown { constructor_id: 0x64f3_6dfc }));
        assert!(matches!(event(deleted), ChatEventAction::Unknown { constructor_id: 0x42e0_47bb }));
    }
}
