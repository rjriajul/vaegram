use crate::raw::{base, types};

/// Proof that you own a wallet key: a signature over a
/// [`TonWalletOwnershipProofChallenge`](super::TonWalletOwnershipProofChallenge).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TonWalletOwnershipProof {
    /// Public key of the wallet that made the signature.
    pub public_key: Vec<u8>,
    /// Unix time the proof was made.
    pub timestamp: i32,
    /// Signature over the challenge, made with the wallet's private key.
    pub signature: Vec<u8>,
}

impl TonWalletOwnershipProof {
    /// Creates a proof from the wallet's public key, the Unix time of signing and the signature over the challenge.
    pub fn new(public_key: impl Into<Vec<u8>>, timestamp: i32, signature: impl Into<Vec<u8>>) -> Self {
        TonWalletOwnershipProof { public_key: public_key.into(), timestamp, signature: signature.into() }
    }

    pub(crate) fn to_raw(&self) -> base::WalletOwnershipProof {
        base::WalletOwnershipProof::WalletOwnershipProof(Box::new(types::WalletOwnershipProof {
            timestamp: self.timestamp,
            signature: self.signature.clone(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::TonWalletOwnershipProof;
    use crate::raw::base;

    #[test]
    fn new_writes_its_timestamp_and_signature() {
        let proof = TonWalletOwnershipProof::new(vec![1, 2], 10, vec![3]);
        assert_eq!(proof.public_key, [1, 2]);
        let base::WalletOwnershipProof::WalletOwnershipProof(raw) = proof.to_raw();
        assert_eq!((raw.timestamp, raw.signature), (10, vec![3]));
    }
}
