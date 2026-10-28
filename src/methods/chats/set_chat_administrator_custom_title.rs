use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::PeerRef;
use crate::utils;

impl Client {
    pub async fn set_chat_administrator_custom_title(
        &self,
        chat_id: impl Into<PeerRef>,
        user_id: impl Into<PeerRef>,
        custom_title: impl Into<String>,
    ) -> Result<()> {
        let channel = self.resolve_channel(&chat_id.into()).await?;
        let participant = self.resolve_peer(user_id).await?;
        let user_id =
            utils::input_user(participant.clone()).ok_or(Error::InvalidArgument("only users can be administrators"))?;
        let request = functions::channels::GetParticipant { channel: channel.clone(), participant };
        let base::channels::ChannelParticipant::ChannelParticipant(found) = self.invoke(&request).await?;
        let admin_rights = current_admin_rights(found.participant)?;
        let request =
            functions::channels::EditAdmin { channel, user_id, admin_rights, rank: Some(custom_title.into()) };
        self.invoke(&request).await?;
        Ok(())
    }
}

/// editAdmin replaces the rights too, so resend the ones the admin already has.
/// Sending it for anyone else would promote them instead.
fn current_admin_rights(participant: base::ChannelParticipant) -> Result<base::ChatAdminRights> {
    match participant {
        base::ChannelParticipant::ChannelParticipantAdmin(a) => Ok(a.admin_rights),
        base::ChannelParticipant::ChannelParticipantCreator(c) => Ok(c.admin_rights),
        _ => Err(Error::InvalidArgument("the user is not an administrator, so they can't have a custom title")),
    }
}

#[cfg(test)]
mod tests {
    use super::current_admin_rights;
    use crate::raw::{base, types};

    #[test]
    fn keeps_admin_rights_and_refuses_members() {
        let rights = types::ChatAdminRights::deserialize_bare(&mut &(1u32 << 7).to_le_bytes()[..]).unwrap();
        let rights = base::ChatAdminRights::ChatAdminRights(Box::new(rights));
        let admin = types::ChannelParticipantAdmin {
            can_edit: true,
            is_self: false,
            user_id: 7,
            inviter_id: None,
            promoted_by: 1,
            date: 0,
            admin_rights: rights.clone(),
            rank: None,
        };
        let admin = base::ChannelParticipant::ChannelParticipantAdmin(Box::new(admin));
        assert_eq!(current_admin_rights(admin).unwrap(), rights);

        let member = types::ChannelParticipant { user_id: 7, date: 0, subscription_until_date: None, rank: None };
        let member = base::ChannelParticipant::ChannelParticipant(Box::new(member));
        assert!(current_admin_rights(member).is_err());
    }
}
