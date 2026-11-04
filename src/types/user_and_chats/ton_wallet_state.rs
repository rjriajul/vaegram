use crate::Client;
use crate::raw::base;

/// The state of this account's TON wallet. Telegram sends an empty state while it's still creating the wallet or
/// when there is none, so the address, key and balance are optional.
#[derive(Debug, Clone)]
pub struct TonWalletState {
    pub wallet_address: Option<String>,
    pub public_key: Option<Vec<u8>>,
    /// The balance in nanoton.
    pub gram_amount: Option<i64>,
    pub is_being_created: bool,
    pub is_backup_enabled: bool,
    pub can_enable_backup: bool,
    pub can_export_phrase: bool,
    client: Client,
}

impl TonWalletState {
    pub(crate) fn parse(client: &Client, state: &base::WalletState) -> TonWalletState {
        match state {
            base::WalletState::WalletStateEmpty(empty) => TonWalletState {
                wallet_address: None,
                public_key: None,
                gram_amount: None,
                is_being_created: empty.creating,
                is_backup_enabled: false,
                can_enable_backup: false,
                can_export_phrase: false,
                client: client.clone(),
            },
            base::WalletState::WalletState(s) => TonWalletState {
                wallet_address: Some(s.address.clone()),
                public_key: Some(s.public_key.clone()),
                gram_amount: Some(s.balance),
                is_being_created: false,
                is_backup_enabled: s.backup_enabled,
                can_enable_backup: s.can_enable_backup,
                can_export_phrase: s.can_export_phrase,
                client: client.clone(),
            },
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::TonWalletState;
    use crate::Client;
    use crate::raw::{base, types};

    #[test]
    fn parses_empty_and_full_wallet_states() {
        let empty = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: true });
        let state = TonWalletState::parse(&Client::offline(), &empty);
        assert!(state.is_being_created);
        assert_eq!((state.wallet_address, state.public_key, state.gram_amount), (None, None, None));

        let full = types::WalletState {
            backup_enabled: true,
            can_export_phrase: false,
            can_enable_backup: true,
            address: "EQabc".into(),
            public_key: vec![1, 2],
            balance: 5_000,
        };
        let state = TonWalletState::parse(&Client::offline(), &base::WalletState::WalletState(Box::new(full)));
        assert_eq!(state.wallet_address.as_deref(), Some("EQabc"));
        assert_eq!((state.public_key, state.gram_amount), (Some(vec![1, 2]), Some(5_000)));
        assert_eq!(
            (state.is_being_created, state.is_backup_enabled, state.can_enable_backup, state.can_export_phrase),
            (false, true, true, false)
        );
    }
}
