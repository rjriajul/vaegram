use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;

/// A TON wallet address and the user it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserTonWalletAddress {
    /// The user who owns the address. `None` when Telegram sends no user id for it.
    pub user: Option<User>,
    /// The wallet address.
    pub wallet_address: String,
    /// The wallet public key.
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
