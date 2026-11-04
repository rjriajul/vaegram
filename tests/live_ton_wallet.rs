#[allow(dead_code)]
mod support;

// Replacing, deleting and creating wallets change the account irreversibly, so no live test calls them.

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn wallet_state_reads_mine() {
    let Some(kit) = support::Kit::from_env().await else { return };
    match kit.user.client.get_ton_wallet_state().await {
        Ok(state) => assert!(state.is_being_created || state.wallet_address.is_some(), "{state:?}"),
        Err(e) if e.is("WALLET_UNAVAILABLE") => println!("wallets are unavailable for this account: {e}"),
        Err(e) => panic!("{e}"),
    }
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn proof_challenge_has_payload_and_domain() {
    let Some(kit) = support::Kit::from_env().await else { return };
    match kit.user.client.get_ton_wallet_ownership_proof_challenge().await {
        Ok(c) => assert!(!c.payload.is_empty() && !c.domain.is_empty(), "{c:?}"),
        Err(e) if e.is("WALLET_UNAVAILABLE") => println!("wallets are unavailable for this account: {e}"),
        Err(e) => panic!("{e}"),
    }
}
