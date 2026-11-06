use crate::raw::base;

/// What Telegram wants signed to prove you own a wallet key.
///
/// Fetched with
/// [`Client::get_ton_wallet_ownership_proof_challenge`](crate::Client::get_ton_wallet_ownership_proof_challenge).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TonWalletOwnershipProofChallenge {
    /// The payload to sign.
    pub payload: String,
    /// The domain to include in the signed data.
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
