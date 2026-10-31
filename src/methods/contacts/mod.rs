mod add_contact;
mod block_user;
mod delete_contacts;
mod get_blocked_message_senders;
mod get_contacts;
mod get_contacts_count;
mod import_contacts;
mod search_contacts;
mod set_contact_note;
mod unblock_user;

pub use add_contact::AddContact;
pub use set_contact_note::SetContactNote;

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions::contacts;
    use crate::raw::{Identifiable, base, types};

    #[tokio::test]
    async fn contact_calls_reach_the_right_methods() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(9, base::InputPeer::InputPeerUser(Box::new(user)));
        let mut sent = || u32::from_le_bytes(rx.try_recv().expect("no request sent")[..4].try_into().unwrap());

        let _ = client.block_user(9_i64).await;
        assert_eq!(sent(), contacts::Block::CONSTRUCTOR_ID);
        let _ = client.unblock_user(9_i64).await;
        assert_eq!(sent(), contacts::Unblock::CONSTRUCTOR_ID);
        let _ = client.add_contact(9_i64, "Ann").share_phone_number().await;
        assert_eq!(sent(), contacts::AddContact::CONSTRUCTOR_ID);
        let _ = client.delete_contacts([9_i64]).await;
        assert_eq!(sent(), contacts::DeleteContacts::CONSTRUCTOR_ID);
        let _ = client.get_common_chats(9_i64).await;
        assert_eq!(sent(), crate::raw::functions::messages::GetCommonChats::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn deleting_no_contacts_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let result = client.delete_contacts(Vec::<i64>::new()).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }
}
