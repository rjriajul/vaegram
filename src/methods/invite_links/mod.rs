mod approve_all_chat_join_requests;
mod approve_chat_join_request;
mod create_chat_invite_link;
mod create_chat_subscription_invite_link;
mod decline_all_chat_join_requests;
mod decline_chat_join_request;
mod delete_chat_admin_invite_links;
mod delete_chat_invite_link;
mod edit_chat_invite_link;
mod edit_chat_subscription_invite_link;
mod export_chat_invite_link;
mod get_chat_admin_invite_links;
mod get_chat_admin_invite_links_count;
mod get_chat_admins_with_invite_links;
mod get_chat_invite_link;
mod get_chat_invite_link_joiners;
mod get_chat_invite_link_joiners_count;
mod get_chat_join_requests;
mod revoke_chat_invite_link;

pub use create_chat_invite_link::CreateChatInviteLink;
pub use create_chat_subscription_invite_link::CreateChatSubscriptionInviteLink;
pub use edit_chat_invite_link::EditChatInviteLink;
pub use get_chat_admin_invite_links::GetChatAdminInviteLinks;
pub use get_chat_invite_link_joiners::GetChatInviteLinkJoiners;
pub use get_chat_join_requests::GetChatJoinRequests;

#[cfg(test)]
mod link_tests {
    use super::create_chat_invite_link::tests::{CHANNEL_ID, channel, setup};
    use super::get_chat_join_requests::tests::page;
    use crate::errors::Error;
    use crate::raw::functions::messages;
    use crate::raw::{Serializable, base, types};
    use crate::types::User;
    use crate::utils::get_channel_id;

    const LINK: &str = "https://t.me/+abc";

    fn named_user(id: i64, name: &str) -> base::User {
        let mut buf = Vec::new();
        // access_hash and first_name are set.
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        name.to_owned().serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn invite(link: &str, date: i32) -> base::ExportedChatInvite {
        let invite = types::ChatInviteExported {
            revoked: false,
            permanent: false,
            request_needed: false,
            link: link.into(),
            admin_id: 7,
            date,
            start_date: None,
            expire_date: None,
            usage_limit: None,
            usage: None,
            requested: None,
            subscription_expired: None,
            title: Some("t".into()),
            subscription_pricing: None,
        };
        base::ExportedChatInvite::ChatInviteExported(Box::new(invite))
    }

    fn links(count: i32, invites: Vec<base::ExportedChatInvite>) -> Vec<u8> {
        let page = types::messages::ExportedChatInvites { count, invites, users: vec![named_user(7, "Ann")] };
        base::messages::ExportedChatInvites::ExportedChatInvites(Box::new(page)).to_bytes()
    }

    fn joiners() -> messages::GetChatInviteImporters {
        messages::GetChatInviteImporters {
            requested: false,
            subscription_expired: false,
            peer: channel(),
            link: Some(LINK.into()),
            q: None,
            offset_date: 0,
            offset_user: base::InputUser::InputUserEmpty,
            limit: 100,
        }
    }

    fn admin_links(offset: Option<(i32, &str)>, limit: i32) -> messages::GetExportedChatInvites {
        messages::GetExportedChatInvites {
            revoked: false,
            peer: channel(),
            admin_id: base::InputUser::InputUserSelf,
            offset_date: offset.map(|(date, _)| date),
            offset_link: offset.map(|(_, link)| link.into()),
            limit,
        }
    }

    fn user_7() -> base::InputUser {
        base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 2 }))
    }

    #[tokio::test]
    async fn pages_link_joiners_after_the_last_joiner() {
        let (client, mut rx) = setup(vec![page(3, &[(10, 300), (11, 200)]), page(3, &[(12, 100)])]);
        let found = client.get_chat_invite_link_joiners(get_channel_id(CHANNEL_ID), LINK).await.unwrap();
        assert_eq!(found.iter().map(|j| j.user.id).collect::<Vec<_>>(), [10, 11, 12]);
        assert_eq!(rx.try_recv().unwrap(), joiners().to_bytes());
        let user = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 11, access_hash: 110 }));
        let next = messages::GetChatInviteImporters { offset_date: 200, offset_user: user, ..joiners() };
        assert_eq!(rx.try_recv().unwrap(), next.to_bytes());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn no_joiners_take_one_request_and_a_zero_limit_none() {
        let chat = get_channel_id(CHANNEL_ID);
        let (client, mut rx) = setup(vec![page(0, &[])]);
        assert!(client.get_chat_invite_link_joiners(chat, LINK).await.unwrap().is_empty());
        rx.try_recv().unwrap();
        assert!(client.get_chat_invite_link_joiners(chat, LINK).limit(0).await.unwrap().is_empty());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn counts_ask_for_a_single_item() {
        let chat = get_channel_id(CHANNEL_ID);
        let (client, mut rx) = setup(vec![page(42, &[(10, 300)]), links(5, vec![invite(LINK, 1)])]);
        assert_eq!(client.get_chat_invite_link_joiners_count(chat, LINK).await.unwrap(), 42);
        assert_eq!(rx.try_recv().unwrap(), messages::GetChatInviteImporters { limit: 1, ..joiners() }.to_bytes());
        assert_eq!(client.get_chat_admin_invite_links_count(chat, "me", true).await.unwrap(), 5);
        let request = messages::GetExportedChatInvites { revoked: true, ..admin_links(None, 1) };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn looks_up_and_deletes_one_link() {
        let chat = get_channel_id(CHANNEL_ID);
        let found = types::messages::ExportedChatInvite { invite: invite(LINK, 1), users: vec![named_user(7, "Ann")] };
        let found = base::messages::ExportedChatInvite::ExportedChatInvite(Box::new(found));
        let replaced = types::messages::ExportedChatInviteReplaced {
            invite: invite(LINK, 1),
            new_invite: invite("https://t.me/+new", 2),
            users: Vec::new(),
        };
        let replaced = base::messages::ExportedChatInvite::ExportedChatInviteReplaced(Box::new(replaced));
        let public = types::messages::ExportedChatInvite {
            invite: base::ExportedChatInvite::ChatInvitePublicJoinRequests,
            users: Vec::new(),
        };
        let public = base::messages::ExportedChatInvite::ExportedChatInvite(Box::new(public));
        let replies = vec![found.to_bytes(), replaced.to_bytes(), public.to_bytes(), true.to_bytes()];
        let (client, mut rx) = setup(replies);

        let link = client.get_chat_invite_link(chat, LINK).await.unwrap();
        assert_eq!((link.invite_link.as_str(), link.creator.first_name.as_str()), (LINK, "Ann"));
        let request = messages::GetExportedChatInvite { peer: channel(), link: LINK.into() };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
        assert_eq!(client.get_chat_invite_link(chat, LINK).await.unwrap().invite_link, LINK);
        let public = client.get_chat_invite_link(chat, LINK).await;
        assert!(matches!(public, Err(Error::Unexpected(_))), "{public:?}");

        client.delete_chat_invite_link(chat, LINK).await.unwrap();
        // The Replaced and public-requests lookups.
        rx.try_recv().unwrap();
        rx.try_recv().unwrap();
        let request = messages::DeleteExportedChatInvite { peer: channel(), link: LINK.into() };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn pages_admin_links_after_the_last_link() {
        let chat = get_channel_id(CHANNEL_ID);
        let public = base::ExportedChatInvite::ChatInvitePublicJoinRequests;
        let first = links(4, vec![invite("l1", 30), invite("l2", 20), public]);
        let (client, mut rx) = setup(vec![first, links(4, vec![invite("l3", 10)])]);
        let found = client.get_chat_admin_invite_links(chat, "me").limit(3).await.unwrap();
        assert_eq!(found.iter().map(|l| l.invite_link.as_str()).collect::<Vec<_>>(), ["l1", "l2", "l3"]);
        assert_eq!(found[0].creator.first_name, "Ann");
        assert_eq!(rx.try_recv().unwrap(), admin_links(None, 3).to_bytes());
        assert_eq!(rx.try_recv().unwrap(), admin_links(Some((20, "l2")), 1).to_bytes());
        assert!(rx.try_recv().is_err());

        let _ = client.get_chat_admin_invite_links(chat, "me").revoked().limit(3).await;
        let request = messages::GetExportedChatInvites { revoked: true, ..admin_links(None, 3) };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());
    }

    #[tokio::test]
    async fn admin_links_stop_on_an_empty_page_or_a_repeated_cursor() {
        let chat = get_channel_id(CHANNEL_ID);
        let (client, mut rx) = setup(vec![links(9, Vec::new())]);
        assert!(client.get_chat_admin_invite_links(chat, "me").await.unwrap().is_empty());
        rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());

        let same = || links(9, vec![invite("l1", 30)]);
        let (client, mut rx) = setup(vec![same(), same(), same()]);
        client.get_chat_admin_invite_links(chat, "me").await.unwrap();
        assert_eq!(std::iter::from_fn(|| rx.try_recv().ok()).count(), 2);
    }

    #[tokio::test]
    async fn lists_admins_with_their_link_counts() {
        let admin = |admin_id, invites_count, revoked_invites_count| {
            let admin = types::ChatAdminWithInvites { admin_id, invites_count, revoked_invites_count };
            base::ChatAdminWithInvites::ChatAdminWithInvites(Box::new(admin))
        };
        let found = types::messages::ChatAdminsWithInvites {
            admins: vec![admin(7, 3, 1), admin(8, 0, 2)],
            users: vec![named_user(7, "Ann")],
        };
        let found = base::messages::ChatAdminsWithInvites::ChatAdminsWithInvites(Box::new(found));
        let (client, mut rx) = setup(vec![found.to_bytes()]);
        let admins = client.get_chat_admins_with_invite_links(get_channel_id(CHANNEL_ID)).await.unwrap();
        let ann = &admins[0];
        assert_eq!(
            (ann.admin.first_name.as_str(), ann.chat_invite_links_count, ann.revoked_chat_invite_links_count),
            ("Ann", 3, 1)
        );
        // An admin missing from `users` still comes back, by id.
        assert_eq!(admins[1].admin, User { id: 8, ..User::default() });
        assert_eq!((admins[1].chat_invite_links_count, admins[1].revoked_chat_invite_links_count), (0, 2));
        assert_eq!(rx.try_recv().unwrap(), messages::GetAdminsWithInvites { peer: channel() }.to_bytes());
    }

    #[tokio::test]
    async fn deletes_an_admins_revoked_links() {
        let chat = get_channel_id(CHANNEL_ID);
        let (client, mut rx) = setup(vec![true.to_bytes()]);
        client.delete_chat_admin_invite_links(chat, 7).await.unwrap();
        let request = messages::DeleteRevokedExportedChatInvites { peer: channel(), admin_id: user_7() };
        assert_eq!(rx.try_recv().unwrap(), request.to_bytes());

        assert!(client.delete_chat_admin_invite_links(chat, chat).await.is_err());
        assert!(rx.try_recv().is_err());
    }
}
