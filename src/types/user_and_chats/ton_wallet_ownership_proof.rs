use crate::raw::{base, types};

/// Proof that you own a wallet key: a signature over a
/// [`TonWalletOwnershipProofChallenge`](super::TonWalletOwnershipProofChallenge).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TonWalletOwnershipProof {
    /// Public key of the wallet that made the signature.
    pub public_key: Vec<u8>,
    /// Unix time the proof was made.
    pub timestamp: i32,
    /// Signature over the challenge, made with the wallet's private key.
    pub signature: Vec<u8>,
}

impl TonWalletOwnershipProof {
    pub(crate) fn to_raw(&self) -> base::WalletOwnershipProof {
        base::WalletOwnershipProof::WalletOwnershipProof(Box::new(types::WalletOwnershipProof {
            timestamp: self.timestamp,
            signature: self.signature.clone(),
        }))
    }
}
