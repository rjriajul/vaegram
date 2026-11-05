use crate::Client;
use crate::enums::ChatType;
use crate::errors::Result;
use crate::handlers::Listenable;
use crate::methods::{
    Ask, BanChatMember, GetChatMembers, Listen, MuteChat, RestrictChatMember, SetChatPhoto, StopListening,
};
use crate::types::{Chat, ChatMember, ChatPermissions, ChatPrivileges, FailedToAddMember, PeerRef, Text, Upload};

/// Shortcuts for the `Client` methods that take this chat: each one forwards to its method with the chat's id and
/// gives back what the method gives back.
impl Chat {
    /// Returns a user's or bot's first and last name, or a group's or channel's title.
    ///
    /// `None` if the chat carries no name.
    pub fn full_name(&self) -> Option<String> {
        match self.kind {
            ChatType::Private | ChatType::Bot => match (self.first_name.as_deref(), self.last_name.as_deref()) {
                (Some(first), Some(last)) => Some(format!("{first} {last}")),
                (Some(name), None) | (None, Some(name)) => Some(name.to_owned()),
                (None, None) => None,
            },
            _ => self.title.clone(),
        }
    }

    /// Waits for the next update of kind `T` in this chat.
    pub fn listen<'a, T: Listenable>(&self, client: &'a Client) -> Listen<'a, T> {
        client.listen().chat(self.id)
    }

    /// Stops the listeners of kind `T` in this chat.
    pub fn stop_listening<'a, T: Listenable>(&self, client: &'a Client) -> StopListening<'a, T> {
        client.stop_listening().chat(self.id)
    }

    /// Sends `text` to this chat and waits for the answer.
    pub fn ask<'a>(&self, client: &'a Client, text: impl Into<Text>) -> Ask<'a> {
        client.ask(self.id, text)
    }

    /// Moves this chat to the archive with [`Client::archive_chats`].
    pub async fn archive(&self, client: &Client) -> Result<()> {
        client.archive_chats([self.id]).await
    }

    /// Moves this chat out of the archive with [`Client::unarchive_chats`].
    pub async fn unarchive(&self, client: &Client) -> Result<()> {
        client.unarchive_chats([self.id]).await
    }

    /// Renames this group or channel with [`Client::set_chat_title`].
    pub async fn set_title(&self, client: &Client, title: impl Into<String>) -> Result<()> {
        client.set_chat_title(self.id, title).await
    }

    /// Changes the description of this group or channel with [`Client::set_chat_description`].
    pub async fn set_description(&self, client: &Client, description: impl Into<String>) -> Result<()> {
        client.set_chat_description(self.id, description).await
    }

    /// Changes the photo of this group or channel with [`Client::set_chat_photo`].
    pub fn set_photo<'a>(&self, client: &'a Client, photo: impl Into<Upload<'a>>) -> SetChatPhoto<'a> {
        client.set_chat_photo(self.id, photo)
    }

    /// Sets the auto-delete timer of this chat with [`Client::set_chat_ttl`].
    pub async fn set_ttl(&self, client: &Client, ttl_seconds: i32) -> Result<()> {
        client.set_chat_ttl(self.id, ttl_seconds).await
    }

    /// Bans a user from this chat with [`Client::ban_chat_member`].
    pub fn ban_member<'a>(&self, client: &'a Client, user_id: impl Into<PeerRef>) -> BanChatMember<'a> {
        client.ban_chat_member(self.id, user_id)
    }

    /// Lifts the ban on a user with [`Client::unban_chat_member`].
    pub async fn unban_member(&self, client: &Client, user_id: impl Into<PeerRef>) -> Result<()> {
        client.unban_chat_member(self.id, user_id).await
    }

    /// Limits what a user may do in this chat with [`Client::restrict_chat_member`].
    pub fn restrict_member<'a>(
        &self,
        client: &'a Client,
        user_id: impl Into<PeerRef>,
        permissions: ChatPermissions,
    ) -> RestrictChatMember<'a> {
        client.restrict_chat_member(self.id, user_id, permissions)
    }

    /// Gives a user admin rights in this chat with [`Client::promote_chat_member`].
    pub async fn promote_member(
        &self,
        client: &Client,
        user_id: impl Into<PeerRef>,
        privileges: ChatPrivileges,
    ) -> Result<()> {
        client.promote_chat_member(self.id, user_id, privileges).await
    }

    /// Joins this chat with [`Client::join_chat`] and returns the joined [`Chat`].
    pub async fn join(&self, client: &Client) -> Result<Chat> {
        client.join_chat(self.id).await
    }

    /// Leaves this chat with [`Client::leave_chat`].
    pub async fn leave(&self, client: &Client) -> Result<()> {
        client.leave_chat(self.id).await
    }

    /// Returns a new primary invite link for this chat, from [`Client::export_chat_invite_link`].
    pub async fn export_invite_link(&self, client: &Client) -> Result<String> {
        client.export_chat_invite_link(self.id).await
    }

    /// Fetches one member of this chat with [`Client::get_chat_member`].
    pub async fn get_member(&self, client: &Client, user_id: impl Into<PeerRef>) -> Result<ChatMember> {
        client.get_chat_member(self.id, user_id).await
    }

    /// Lists the members of this chat with [`Client::get_chat_members`].
    pub fn get_members<'a>(&self, client: &'a Client) -> GetChatMembers<'a> {
        client.get_chat_members(self.id)
    }

    /// Adds users to this chat with [`Client::add_chat_members`]. Returns the users that couldn't be added.
    pub async fn add_members(
        &self,
        client: &Client,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
        forward_limit: i32,
    ) -> Result<Vec<FailedToAddMember>> {
        client.add_chat_members(self.id, user_ids, forward_limit).await
    }

    /// Marks this chat as unread with [`Client::mark_chat_unread`].
    pub async fn mark_unread(&self, client: &Client) -> Result<()> {
        client.mark_chat_unread(self.id).await
    }

    /// Turns forwarding and saving restrictions on or off with [`Client::set_chat_protected_content`].
    pub async fn set_protected_content(&self, client: &Client, enabled: bool) -> Result<()> {
        client.set_chat_protected_content(self.id, enabled).await
    }

    /// Unpins every message in this chat with [`Client::unpin_all_chat_messages`].
    pub async fn unpin_all_messages(&self, client: &Client) -> Result<()> {
        client.unpin_all_chat_messages(self.id).await
    }

    /// Mutes this chat with [`Client::mute_chat`].
    pub fn mute<'a>(&self, client: &'a Client) -> MuteChat<'a> {
        client.mute_chat(self.id)
    }

    /// Unmutes this chat with [`Client::unmute_chat`].
    pub async fn unmute(&self, client: &Client) -> Result<bool> {
        client.unmute_chat(self.id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::dispatcher::Entities;
    use crate::enums::ChatType;
    use crate::raw::functions::{account, channels, folders, messages};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::types::{Chat, ChatPermissions, ChatPrivileges, Message};
    use crate::utils::{get_channel_id, get_peer};

    const USER: i64 = 7;

    fn cached(client: &Client) -> Chat {
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        let id = get_channel_id(9);
        let mut peers = client.0.peers.lock().unwrap();
        peers.insert(id, base::InputPeer::InputPeerChannel(Box::new(channel)));
        peers.insert(
            USER,
            base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 })),
        );
        Chat::parse(&get_peer(id), &Entities::default(), true)
    }

    #[tokio::test]
    async fn chat_helpers_target_their_chat() {
        let (client, mut rx) = Client::recording();
        let chat = cached(&client);
        let rx = &mut rx;
        let privileges = ChatPrivileges::default();

        assert_eq!(sent(rx, chat.archive(&client)).await, folders::EditPeerFolders::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.unarchive(&client)).await, folders::EditPeerFolders::CONSTRUCTOR_ID);
        let _ = chat.set_title(&client, "t").await;
        let channel = base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 1 }));
        let edit = channels::EditTitle { channel: channel.clone(), title: "t".into() };
        assert_eq!(rx.try_recv().unwrap(), edit.to_bytes());
        assert_eq!(sent(rx, chat.set_description(&client, "d")).await, messages::EditChatAbout::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.set_ttl(&client, 60)).await, messages::SetHistoryTTL::CONSTRUCTOR_ID);
        let _ = chat.ban_member(&client, USER).await;
        let mut rights = ChatPermissions::default().write(0);
        rights.view_messages = true;
        let ban = channels::EditBanned {
            channel,
            participant: base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser {
                user_id: USER,
                access_hash: 2,
            })),
            banned_rights: base::ChatBannedRights::ChatBannedRights(Box::new(rights)),
        };
        assert_eq!(rx.try_recv().unwrap(), ban.to_bytes());
        assert_eq!(sent(rx, chat.unban_member(&client, USER)).await, channels::EditBanned::CONSTRUCTOR_ID);
        let restrict = chat.restrict_member(&client, USER, ChatPermissions::default());
        assert_eq!(sent(rx, restrict).await, channels::EditBanned::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.promote_member(&client, USER, privileges)).await, channels::EditAdmin::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.join(&client)).await, channels::JoinChannel::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.leave(&client)).await, channels::LeaveChannel::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.export_invite_link(&client)).await, messages::ExportChatInvite::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.get_member(&client, USER)).await, channels::GetParticipant::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.get_members(&client).limit(1)).await, channels::GetParticipants::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.add_members(&client, [USER], 0)).await, channels::InviteToChannel::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.mark_unread(&client)).await, messages::MarkDialogUnread::CONSTRUCTOR_ID);
        assert_eq!(
            sent(rx, chat.set_protected_content(&client, true)).await,
            messages::ToggleNoForwards::CONSTRUCTOR_ID
        );
        assert_eq!(sent(rx, chat.unpin_all_messages(&client)).await, messages::UnpinAllMessages::CONSTRUCTOR_ID);
        let _ = chat.mute(&client).await;
        let peer =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        let peer = base::InputNotifyPeer::InputNotifyPeer(Box::new(types::InputNotifyPeer { peer }));
        let settings = types::InputPeerNotifySettings {
            show_previews: None,
            silent: None,
            mute_until: Some(i32::MAX),
            sound: None,
            stories_muted: None,
            stories_hide_sender: None,
            stories_sound: None,
        };
        let settings = base::InputPeerNotifySettings::InputPeerNotifySettings(Box::new(settings));
        assert_eq!(rx.try_recv().unwrap(), account::UpdateNotifySettings { peer, settings }.to_bytes());
        assert_eq!(sent(rx, chat.unmute(&client)).await, account::UpdateNotifySettings::CONSTRUCTOR_ID);
        assert_eq!(sent(rx, chat.ask(&client, "hi")).await, messages::SendMessage::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn listening_helpers_scope_to_the_chat() {
        let (client, _rx) = Client::recording();
        let chat = cached(&client);
        assert_eq!(chat.stop_listening::<Message>(&client).await.unwrap(), 0);
    }

    #[test]
    fn full_name_is_the_person_or_the_title() {
        let mut chat = Chat::parse(&get_peer(USER), &Entities::default(), false);
        chat.kind = ChatType::Private;
        chat.first_name = Some("Ada".into());
        chat.last_name = Some("Lovelace".into());
        assert_eq!(chat.full_name().as_deref(), Some("Ada Lovelace"));
        chat.last_name = None;
        assert_eq!(chat.full_name().as_deref(), Some("Ada"));
        chat.kind = ChatType::Group;
        chat.title = Some("Friends".into());
        assert_eq!(chat.full_name().as_deref(), Some("Friends"));
    }
}
