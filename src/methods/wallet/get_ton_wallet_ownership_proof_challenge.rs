use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::TonWalletOwnershipProofChallenge;

impl Client {
    /// Returns the challenge to sign to prove ownership of a TON wallet.
    pub async fn get_ton_wallet_ownership_proof_challenge(&self) -> Result<TonWalletOwnershipProofChallenge> {
        Ok(TonWalletOwnershipProofChallenge::parse(self.invoke(&functions::wallet::GetProofChallenge {}).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn challenge_keeps_payload_domain_expiry() {
        let raw = types::wallet::ProofChallenge { payload: "p".into(), expires: 99, domain: "d.org".into() };
        let reply = base::wallet::ProofChallenge::ProofChallenge(Box::new(raw)).to_bytes();
        let (client, _rx) = Client::scripted(vec![Ok(reply)]);
        let challenge = client.get_ton_wallet_ownership_proof_challenge().await.unwrap();
        assert_eq!(
            (challenge.payload.as_str(), challenge.domain.as_str(), challenge.expiration_date),
            ("p", "d.org", 99)
        );
    }
}
