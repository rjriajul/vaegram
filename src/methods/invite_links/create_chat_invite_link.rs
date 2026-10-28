use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{ChatInviteLink, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct CreateChatInviteLink<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    name: Option<String>,
    expire_date: Option<i32>,
    member_limit: Option<i32>,
    creates_join_request: bool,
}

impl Client {
    pub fn create_chat_invite_link(&self, chat_id: impl Into<PeerRef>) -> CreateChatInviteLink<'_> {
        CreateChatInviteLink {
            client: self,
            chat_id: chat_id.into(),
            name: None,
            expire_date: None,
            member_limit: None,
            creates_join_request: false,
        }
    }
}

impl CreateChatInviteLink<'_> {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn expire_date(mut self, date: i32) -> Self {
        self.expire_date = Some(date);
        self
    }

    pub fn member_limit(mut self, limit: i32) -> Self {
        self.member_limit = Some(limit);
        self
    }

    /// Users joining through the link need approval; can't be combined with `member_limit`.
    pub fn creates_join_request(mut self) -> Self {
        self.creates_join_request = true;
        self
    }

    async fn send(self) -> Result<ChatInviteLink> {
        let request = functions::messages::ExportChatInvite {
            legacy_revoke_permanent: false,
            request_needed: self.creates_join_request,
            peer: self.client.resolve_peer(self.chat_id).await?,
            expire_date: self.expire_date,
            usage_limit: self.member_limit,
            title: self.name,
            subscription_pricing: None,
        };
        ChatInviteLink::parse(self.client.invoke(&request).await?, &Entities::default())
    }
}

into_future!(CreateChatInviteLink, ChatInviteLink);

#[cfg(test)]
pub(crate) mod tests {
    use std::future::IntoFuture;

    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::Client;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::utils::get_channel_id;

    pub(crate) const CHANNEL_ID: i64 = 9;
    const USER: i64 = 7;
    const LINK: &str = "https://t.me/+abc";

    pub(crate) fn channel() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: CHANNEL_ID, access_hash: 1 }))
    }

    pub(crate) fn setup(answers: Vec<Vec<u8>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(answers.into_iter().map(Ok).collect());
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: USER, access_hash: 2 }));
        client.0.peers.lock().unwrap().extend([(get_channel_id(CHANNEL_ID), channel()), (USER, user)]);
        (client, rx)
    }

    async fn sent<F: IntoFuture>(rx: &mut UnboundedReceiver<Vec<u8>>, call: F) -> Vec<u8> {
        let _ = call.await;
        rx.try_recv().expect("no request sent")
    }

    fn export() -> messages::ExportChatInvite {
        messages::ExportChatInvite {
            legacy_revoke_permanent: false,
            request_needed: false,
            peer: channel(),
            expire_date: None,
            usage_limit: None,
            title: None,
            subscription_pricing: None,
        }
    }

    fn edit(revoked: bool) -> messages::EditExportedChatInvite {
        messages::EditExportedChatInvite {
            revoked,
            peer: channel(),
            link: LINK.into(),
            expire_date: None,
            usage_limit: None,
            request_needed: None,
            title: None,
        }
    }

    fn hide(approved: bool) -> messages::HideChatJoinRequest {
        let user = types::InputUser { user_id: USER, access_hash: 2 };
        messages::HideChatJoinRequest { approved, peer: channel(), user_id: base::InputUser::InputUser(Box::new(user)) }
    }

    #[tokio::test]
    async fn builds_requests() {
        let (client, mut rx) = setup(Vec::new());
        let rx = &mut rx;
        let chat = get_channel_id(CHANNEL_ID);

        let request = messages::ExportChatInvite { legacy_revoke_permanent: true, ..export() };
        assert_eq!(sent(rx, client.export_chat_invite_link(chat)).await, request.to_bytes());

        let request = messages::ExportChatInvite {
            request_needed: true,
            expire_date: Some(100),
            title: Some("n".into()),
            ..export()
        };
        let call = client.create_chat_invite_link(chat).name("n").expire_date(100).creates_join_request();
        assert_eq!(sent(rx, call).await, request.to_bytes());

        let pricing = types::StarsSubscriptionPricing { period: 2_592_000, amount: 5 };
        let request = messages::ExportChatInvite {
            subscription_pricing: Some(base::StarsSubscriptionPricing::StarsSubscriptionPricing(Box::new(pricing))),
            title: Some("n".into()),
            ..export()
        };
        let call = client.create_chat_subscription_invite_link(chat, 2_592_000, 5).name("n");
        assert_eq!(sent(rx, call).await, request.to_bytes());

        let request =
            messages::EditExportedChatInvite { usage_limit: Some(0), request_needed: Some(false), ..edit(false) };
        let call = client.edit_chat_invite_link(chat, LINK).member_limit(0).creates_join_request(false);
        assert_eq!(sent(rx, call).await, request.to_bytes());

        let request = messages::EditExportedChatInvite { title: Some("n".into()), ..edit(false) };
        assert_eq!(sent(rx, client.edit_chat_subscription_invite_link(chat, LINK, "n")).await, request.to_bytes());
        assert_eq!(sent(rx, client.revoke_chat_invite_link(chat, LINK)).await, edit(true).to_bytes());

        assert_eq!(sent(rx, client.approve_chat_join_request(chat, USER)).await, hide(true).to_bytes());
        assert_eq!(sent(rx, client.decline_chat_join_request(chat, USER)).await, hide(false).to_bytes());
        let request = messages::HideAllChatJoinRequests { approved: true, peer: channel(), link: Some(LINK.into()) };
        assert_eq!(sent(rx, client.approve_all_chat_join_requests(chat, Some(LINK))).await, request.to_bytes());
        let request = messages::HideAllChatJoinRequests { approved: false, peer: channel(), link: None };
        assert_eq!(sent(rx, client.decline_all_chat_join_requests(chat, None)).await, request.to_bytes());
    }
}
