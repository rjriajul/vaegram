use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserTonWalletAddress {
    /// Telegram leaves it out when the address doesn't belong to a user it can show.
    pub user: Option<User>,
    pub wallet_address: String,
    pub public_key: Vec<u8>,
}

impl UserTonWalletAddress {
    pub(crate) fn parse_all(client: &Client, addresses: base::wallet::UserAddresses) -> Vec<UserTonWalletAddress> {
        let base::wallet::UserAddresses::UserAddresses(list) = addresses;
        let list = *list;
        let entities = Entities::new(client, list.users, Vec::new());
        list.addresses
            .into_iter()
            .map(|a| {
                let base::WalletUserAddress::WalletUserAddress(a) = a;
                let a = *a;
                UserTonWalletAddress {
                    user: a.user_id.map(|id| User::parse(id, &entities)),
                    wallet_address: a.address,
                    public_key: a.public_key,
                }
            })
            .collect()
    }
}
