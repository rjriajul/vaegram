use vaegram::raw::functions;
use vaegram::{Client, Error};

async fn user_client() -> Option<Client> {
    let (Ok(session), Ok(api_id)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_API_ID")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION, VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return None;
    };
    let api_hash = std::env::var("VAEGRAM_API_HASH").expect("set VAEGRAM_API_HASH");
    Some(Client::builder(session, api_id.parse().unwrap(), api_hash).connect().await.unwrap())
}

fn rpc_is(error: &Error, name: &str) -> bool {
    error.rpc().is_some_and(|e| e.is(name))
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn takeout_exports_inside_the_session() {
    let Some(client) = user_client().await else { return };
    let outside = client.invoke(&functions::contacts::GetSaved {}).await.unwrap_err();
    assert!(rpc_is(&outside, "TAKEOUT_REQUIRED"), "{outside}");

    let session = match client.takeout().contacts().await {
        Ok(session) => session,
        Err(e) if rpc_is(&e, "TAKEOUT_INIT_DELAY") => {
            eprintln!("skipped: takeout opens in {:?}; confirm it in another app", e.flood_wait());
            return;
        }
        Err(e) => panic!("{e}"),
    };
    session.client().invoke(&functions::contacts::GetSaved {}).await.unwrap();
    session.client().invoke(&functions::messages::GetSplitRanges {}).await.unwrap();
    assert!(session.finish(true).await.unwrap());
}
