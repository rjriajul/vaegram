//! Invite links and join requests.

use serde_json::{Map, Value, json};

use super::super::Transport;
use super::{decode, no_updates, put, user_of};
use crate::Client;
use crate::bot_api::convert::{Peers, int};
use crate::bot_api::raw::types as bot;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions as f, types};
use crate::session::internals::now;
use crate::types::ChatInviteLink;

const EXPORT_INVITE: u32 = <f::messages::ExportChatInvite as Identifiable>::CONSTRUCTOR_ID;
const EDIT_INVITE: u32 = <f::messages::EditExportedChatInvite as Identifiable>::CONSTRUCTOR_ID;
const HIDE_JOIN_REQUEST: u32 = <f::messages::HideChatJoinRequest as Identifiable>::CONSTRUCTOR_ID;
const JOIN_CHAT_RESULTS: u32 = <f::bots::SetJoinChatResults as Identifiable>::CONSTRUCTOR_ID;

impl Transport {
    pub(super) async fn answer_chat_invites(&self, id: u32, body: &[u8]) -> Option<Result<Vec<u8>>> {
        Some(match id {
            EXPORT_INVITE => self.export_invite(body).await,
            EDIT_INVITE => self.edit_invite(body).await,
            HIDE_JOIN_REQUEST => self.hide_join_request(body).await,
            JOIN_CHAT_RESULTS => self.join_chat_results(body).await,
            _ => return None,
        })
    }

    /// A subscription price is `createChatSubscriptionInviteLink`; a new primary link and nothing else is
    /// `exportChatInviteLink`, which answers the link alone; anything else is `createChatInviteLink`.
    async fn export_invite(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::ExportChatInvite::from_bytes(body)?;
        let me = self.me();
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        put(&mut body, "name", r.title.clone());
        let mut peers = Peers::new(me);
        let invite =
            if let Some(base::StarsSubscriptionPricing::StarsSubscriptionPricing(pricing)) = &r.subscription_pricing {
                body.insert("subscription_period".into(), pricing.period.into());
                body.insert("subscription_price".into(), pricing.amount.into());
                invite(decode(self.call("createChatSubscriptionInviteLink", Value::Object(body)).await?)?, &mut peers)
            } else if r.legacy_revoke_permanent
                && !r.request_needed
                && r.expire_date.is_none()
                && r.usage_limit.is_none()
                && r.title.is_none()
            {
                let link = self.call("exportChatInviteLink", Value::Object(body)).await?;
                let link = link.as_str().ok_or(Error::Unexpected("exportChatInviteLink returned no link"))?;
                let mut invite = types::ChatInviteExported::new(link.to_owned(), me, int(now()));
                invite.permanent = true;
                base::ExportedChatInvite::ChatInviteExported(Box::new(invite))
            } else {
                put(&mut body, "expire_date", r.expire_date);
                put(&mut body, "member_limit", r.usage_limit);
                if r.request_needed {
                    body.insert("creates_join_request".into(), true.into());
                }
                invite(decode(self.call("createChatInviteLink", Value::Object(body)).await?)?, &mut peers)
            };
        Ok(invite.to_bytes())
    }

    /// `revokeChatInviteLink`, or `editChatInviteLink`. Revoking a primary link answers the revoked link alone,
    /// without the one that replaces it.
    async fn edit_invite(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::EditExportedChatInvite::from_bytes(body)?;
        let mut body = Map::new();
        body.insert("chat_id".into(), self.chat_id(&r.peer)?.into());
        body.insert("invite_link".into(), r.link.into());
        let link = if r.revoked {
            self.call("revokeChatInviteLink", Value::Object(body)).await?
        } else {
            put(&mut body, "name", r.title);
            put(&mut body, "expire_date", r.expire_date);
            put(&mut body, "member_limit", r.usage_limit);
            put(&mut body, "creates_join_request", r.request_needed);
            self.call("editChatInviteLink", Value::Object(body)).await?
        };
        let mut peers = Peers::new(self.me());
        let invite = invite(decode(link)?, &mut peers);
        Ok(types::messages::ExportedChatInvite { invite, users: peers.users }.to_bytes())
    }

    /// `edit_chat_subscription_invite_link`: in TL a rename like any other link's; the Bot API keeps a method of its
    /// own for subscription links.
    pub(crate) async fn edit_chat_subscription_invite_link(
        &self,
        client: &Client,
        chat_id: i64,
        invite_link: String,
        name: String,
    ) -> Result<ChatInviteLink> {
        let body = json!({ "chat_id": chat_id, "invite_link": invite_link, "name": name });
        let link = self.call("editChatSubscriptionInviteLink", body).await?;
        let mut peers = Peers::new(self.me());
        let invite = invite(decode(link)?, &mut peers);
        ChatInviteLink::parse(invite, &Entities::new(client, peers.users, Vec::new()))
    }

    async fn hide_join_request(&self, body: &[u8]) -> Result<Vec<u8>> {
        let r = f::messages::HideChatJoinRequest::from_bytes(body)?;
        let me = self.me();
        let body = json!({ "chat_id": self.chat_id(&r.peer)?, "user_id": user_of(&r.user_id, me)? });
        let method = if r.approved { "approveChatJoinRequest" } else { "declineChatJoinRequest" };
        self.call(method, body).await?;
        Ok(no_updates(me))
    }

    async fn join_chat_results(&self, body: &[u8]) -> Result<Vec<u8>> {
        use base::JoinChatBotResult as R;
        let r = f::bots::SetJoinChatResults::from_bytes(body)?;
        let query_id = r.query_id.to_string();
        let result = match r.result {
            R::JoinChatBotResultApproved => "approve",
            R::JoinChatBotResultDeclined => "decline",
            R::JoinChatBotResultQueued => "queue",
            R::JoinChatBotResultWebView(view) => {
                let body = json!({ "chat_join_request_query_id": query_id, "web_app_url": view.url });
                self.call("sendChatJoinRequestWebApp", body).await?;
                return Ok(true.to_bytes());
            }
        };
        let body = json!({ "chat_join_request_query_id": query_id, "result": result });
        self.call("answerChatJoinRequestQuery", body).await?;
        Ok(true.to_bytes())
    }
}

/// A Bot API `ChatInviteLink` as TL's, its creator added to `peers`. The Bot API gives no creation date, so it is
/// the time of the answer, and no count of who joined.
fn invite(link: bot::ChatInviteLink, peers: &mut Peers) -> base::ExportedChatInvite {
    peers.user(&link.creator);
    let mut invite = types::ChatInviteExported::new(link.invite_link, link.creator.id, int(now()));
    invite.revoked = link.is_revoked;
    invite.permanent = link.is_primary;
    invite.request_needed = link.creates_join_request;
    invite.title = link.name;
    invite.expire_date = link.expire_date.map(int);
    invite.usage_limit = link.member_limit.map(int);
    invite.requested = link.pending_join_request_count.map(int);
    if let (Some(period), Some(amount)) = (link.subscription_period, link.subscription_price) {
        let pricing = types::StarsSubscriptionPricing { period: int(period), amount };
        invite.subscription_pricing = Some(base::StarsSubscriptionPricing::StarsSubscriptionPricing(Box::new(pricing)));
    }
    base::ExportedChatInvite::ChatInviteExported(Box::new(invite))
}
