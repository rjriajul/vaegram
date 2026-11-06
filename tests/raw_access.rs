//! The raw layer stays reachable from outside the crate: `raw` (with `raw::core`), `Client::invoke`,
//! `Client::invoke_with_timeout`, `Client::on_raw_update` with `RawUpdate`, and the chat id converters in `utils`.
//! Crates built on raw TL (a Bot API layer, for one) depend on exactly these.

#[allow(dead_code)]
#[path = "../benches/support/mockdc.rs"]
mod mockdc;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use mockdc::{MockDc, SELF_ID};
use vaegram::Client;
use vaegram::raw::core::msg_container;
use vaegram::raw::{Deserializable, Identifiable, LAYER, RemoteCall, Serializable, base, functions, types};
use vaegram::session::Dc;
use vaegram::types::{RawUpdate, User};
use vaegram::utils::{get_channel_id, get_peer, get_peer_id};

#[tokio::test(flavor = "multi_thread")]
async fn invoke_and_raw_updates_work_from_outside_the_crate() {
    let mock = MockDc::start(Duration::ZERO);
    let client = Client::builder("mock", 1, "mock").session_string(mock.session_string()).connect().await.unwrap();

    let base::Pong::Pong(pong) = client.invoke(&functions::Ping { ping_id: 7 }).await.unwrap();
    assert_eq!(pong.ping_id, 7);
    let ping = functions::Ping { ping_id: 8 };
    let base::Pong::Pong(pong) = client.invoke_with_timeout(&ping, Duration::from_secs(5)).await.unwrap();
    assert_eq!(pong.ping_id, 8);

    let me: User = client.get_me().await.unwrap();
    assert_eq!((me.id, me.is_self), (SELF_ID, true));

    let seen = Arc::new(AtomicU32::new(0));
    let counted = seen.clone();
    client.on_raw_update(move |_, update: RawUpdate| {
        assert_ne!(update.constructor_id(), 0);
        counted.fetch_add(1, Ordering::Relaxed);
        async { Ok(()) }
    });
    let states = mock.stats.states.load(Ordering::Relaxed);
    let runner = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    while mock.stats.states.load(Ordering::Relaxed) == states {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    mock.push(2);
    tokio::time::timeout(Duration::from_secs(5), async {
        while seen.load(Ordering::Relaxed) < 2 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("pushed updates didn't reach the raw handler");
    runner.abort();
}

#[test]
fn raw_types_and_core_are_public() {
    fn round_trip<T: Serializable + Deserializable>(value: &T) -> T {
        T::from_bytes(&value.to_bytes()).unwrap()
    }
    fn returns<R: RemoteCall>(_: &R) {}

    let pong = base::Pong::Pong(Box::new(types::Pong { msg_id: 1, ping_id: 2 }));
    let base::Pong::Pong(back) = round_trip(&pong);
    assert_eq!((back.msg_id, back.ping_id), (1, 2));
    assert!(pong.to_bytes().starts_with(&types::Pong::CONSTRUCTOR_ID.to_le_bytes()));
    returns(&functions::help::GetConfig {});
    assert!(std::hint::black_box(LAYER) > 0);

    let body = pong.to_bytes();
    let mut container = msg_container::ID.to_bytes();
    1u32.serialize(&mut container);
    5i64.serialize(&mut container);
    1i32.serialize(&mut container);
    (body.len() as u32).serialize(&mut container);
    container.extend_from_slice(&body);
    let messages = msg_container::parse(&container).unwrap();
    assert_eq!((messages.len(), messages[0].msg_id, messages[0].body), (1, 5, &body[..]));
}

#[test]
fn chat_id_converters_are_public() {
    assert_eq!(get_channel_id(1234), -1_000_000_001_234);
    for id in [777, -42, get_channel_id(1234)] {
        assert_eq!(get_peer_id(&get_peer(id)), id);
    }
}

#[test]
fn connection_targets_are_public() {
    let dc = Dc::new(2, "149.154.167.51:443".parse().unwrap()).test(true);
    assert_eq!((dc.id, dc.wire_id()), (2, 10002));
}
