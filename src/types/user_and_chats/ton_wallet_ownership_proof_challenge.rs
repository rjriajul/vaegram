use crate::raw::base;

/// What Telegram wants signed to prove you own a wallet key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TonWalletOwnershipProofChallenge {
    pub payload: String,
    pub domain: String,
    /// Unix time after which the challenge is no longer accepted.
    pub expiration_date: i32,
}

impl TonWalletOwnershipProofChallenge {
    pub(crate) fn parse(challenge: base::wallet::ProofChallenge) -> TonWalletOwnershipProofChallenge {
        let base::wallet::ProofChallenge::ProofChallenge(c) = challenge;
        TonWalletOwnershipProofChallenge { payload: c.payload, domain: c.domain, expiration_date: c.expires }
    }
}
