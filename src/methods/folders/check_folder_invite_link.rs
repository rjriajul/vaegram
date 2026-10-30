use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::FolderInviteLinkInfo;

impl Client {
    pub async fn check_folder_invite_link(&self, invite_link: &str) -> Result<FolderInviteLinkInfo> {
        let invite =
            self.invoke(&functions::chatlists::CheckChatlistInvite { slug: slug(invite_link)?.into() }).await?;
        Ok(FolderInviteLinkInfo::parse(self, invite))
    }
}

pub(super) fn slug(link: &str) -> Result<&str> {
    let slug = link.rsplit_once(['/', '=']).map_or(link, |(_, slug)| slug);
    if slug.is_empty() {
        return Err(Error::InvalidArgument("the folder invite link has no slug"));
    }
    Ok(slug)
}
