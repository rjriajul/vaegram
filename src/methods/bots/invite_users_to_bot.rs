use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Invites users to the private chat with a bot you own, at most 10 at a time, the way a
    /// `https://t.me/botinvite/<bot_username>` link does.
    ///
    /// The bot can then message the invited users. Each gets a
    /// [`WriteAccessAllowed`](crate::types::WriteAccessAllowed) service message naming you as the
    /// [`inviter`](crate::types::WriteAccessAllowed::inviter).
    pub async fn invite_users_to_bot(
        &self,
        bot_id: impl Into<PeerRef>,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<()> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let users = self.resolve_users(user_ids).await?;
        accepted(self.invoke(&functions::messages::InviteUsersToBot { bot, users }).await?)
    }
}
