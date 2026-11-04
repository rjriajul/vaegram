use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{ChatPreview, PeerRef};

impl Client {
    /// Reads what an invite link shows about its chat without joining it. `chat` is filled when the account is
    /// already a member or may peek into the chat.
    pub async fn get_chat_preview(&self, invite_link: &str) -> Result<ChatPreview> {
        let PeerRef::Invite(hash) = PeerRef::from(invite_link) else {
            return Err(Error::InvalidArgument("get_chat_preview takes an invite link"));
        };
        let invite = self.invoke(&functions::messages::CheckChatInvite { hash: hash.into() }).await?;
        ChatPreview::parse(self, invite)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::ChatType;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, types};

    fn invite() -> types::ChatInvite {
        types::ChatInvite {
            channel: true,
            broadcast: false,
            public: false,
            megagroup: true,
            request_needed: true,
            verified: false,
            scam: false,
            fake: false,
            can_refulfill_subscription: false,
            title: "Cats".into(),
            about: Some("all cats".into()),
            photo: base::Photo::PhotoEmpty(types::PhotoEmpty { id: 0 }),
            participants_count: 42,
            participants: None,
            color: 0,
            subscription_pricing: Some(base::StarsSubscriptionPricing::StarsSubscriptionPricing(Box::new(
                types::StarsSubscriptionPricing { period: 2_592_000, amount: 50 },
            ))),
            subscription_form_id: None,
            bot_verification: None,
        }
    }

    #[tokio::test]
    async fn preview_reads_an_unjoined_invite() {
        let answer = base::ChatInvite::ChatInvite(Box::new(invite())).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(answer)]);
        let preview = client.get_chat_preview("https://t.me/+AbCdEf").await.unwrap();
        assert!(rx.try_recv().is_ok());
        assert_eq!((preview.title.as_str(), preview.kind, preview.members_count), ("Cats", ChatType::Supergroup, 42));
        assert_eq!(preview.about.as_deref(), Some("all cats"));
        assert!(preview.join_by_request && preview.chat.is_none() && preview.photo.is_none());
        assert_eq!((preview.subscription_star_count, preview.subscription_period), (Some(50), Some(2_592_000)));
    }

    #[tokio::test]
    async fn joined_invite_fills_chat() {
        let group = types::Chat {
            creator: false,
            left: false,
            deactivated: false,
            call_active: false,
            call_not_empty: false,
            noforwards: false,
            id: 5,
            title: "friends".into(),
            photo: base::ChatPhoto::ChatPhotoEmpty,
            participants_count: 3,
            date: 0,
            version: 1,
            migrated_to: None,
            admin_rights: None,
            default_banned_rights: None,
        };
        let already = types::ChatInviteAlready { chat: base::Chat::Chat(Box::new(group)) };
        let answer = base::ChatInvite::ChatInviteAlready(Box::new(already)).to_bytes();
        let (client, _rx) = Client::scripted(vec![Ok(answer)]);
        let preview = client.get_chat_preview("t.me/+AbCdEf").await.unwrap();
        let chat = preview.chat.expect("the chat");
        assert_eq!((chat.id, preview.title.as_str(), preview.members_count), (-5, "friends", 3));
    }

    #[tokio::test]
    async fn non_invite_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        for link in ["@cats", "t.me/cats", "12345"] {
            let result = client.get_chat_preview(link).await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{link}: {result:?}");
        }
        assert!(rx.try_recv().is_err());
    }
}
