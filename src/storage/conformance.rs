//! What every engine must do, checked the same way for each: `check(&engine).await` in its tests.

use std::collections::HashMap;
use std::net::SocketAddr;

use super::{PeerRecord, SessionData, Storage, StoredState, UpdateState};
use crate::session::Salt;

type Fields = (i32, bool, Option<SocketAddr>, i32, [u8; 256], i64, bool);

fn fields(s: &SessionData) -> Fields {
    (s.dc_id, s.test_mode, s.addr, s.api_id, s.auth_key, s.user_id, s.is_bot)
}

fn peer(id: i64, usernames: &[&str], phone: Option<&str>, updated: i64) -> PeerRecord {
    let mut p = PeerRecord::new(id, id.wrapping_mul(7));
    p.usernames = usernames.iter().map(|u| u.to_string()).collect();
    (p.phone_number, p.updated) = (phone.map(str::to_owned), updated);
    p
}

/// Runs on an engine that holds nothing yet and leaves it empty.
pub(crate) async fn check(engine: &dyn Storage) {
    engine.open().await.unwrap();
    assert!(engine.load_session().await.unwrap().is_none(), "a new engine has a session");
    assert!(engine.load_state().await.unwrap().is_none(), "a new engine has a state");
    assert!(engine.export_peers(None).await.unwrap().is_empty(), "a new engine has peers");

    let mut session = SessionData::new(4, std::array::from_fn(|i| i as u8));
    (session.api_id, session.test_mode, session.addr) = (12345, true, Some("[2001:db8::1]:443".parse().unwrap()));
    engine.save_session(&session).await.unwrap();
    (session.user_id, session.is_bot, session.addr) = (777000, true, None);
    engine.save_session(&session).await.unwrap();
    let loaded = engine.load_session().await.unwrap().map(|s| fields(&s));
    assert_eq!(loaded, Some(fields(&session)), "the last session wins");

    let update = UpdateState { pts: 10, qts: 2, date: 1_700_000_000, seq: 5, channels: HashMap::from([(1234, 99)]) };
    let state = StoredState::new(777000, Salt { value: -7, time_offset: 3 }, Some(update));
    engine.save_state(&state).await.unwrap();
    assert_eq!(engine.load_state().await.unwrap(), Some(state.clone()));
    let without_update = StoredState::new(777000, Salt::default(), None);
    engine.save_state(&without_update).await.unwrap();
    assert_eq!(engine.load_state().await.unwrap(), Some(without_update.clone()));
    engine.save_state(&state).await.unwrap();

    let alice = peer(100, &["alice"], Some("15550001"), 10);
    let channel = peer(-1_000_000_001_234, &["news", "news_backup"], None, 20);
    engine.update_peers(&[alice.clone(), channel.clone()]).await.unwrap();
    engine.update_peers(&[]).await.unwrap();
    assert_eq!(engine.get_peer_by_id(100).await.unwrap(), Some(alice.clone()));
    assert_eq!(engine.get_peer_by_id(101).await.unwrap(), None);
    assert_eq!(engine.get_peer_by_username("news_backup").await.unwrap(), Some(channel.clone()));
    assert_eq!(engine.get_peer_by_phone_number("15550001").await.unwrap(), Some(alice.clone()));
    assert_eq!(engine.export_peers(None).await.unwrap(), [channel.clone(), alice.clone()], "newest first");
    assert_eq!(engine.export_peers(Some(1)).await.unwrap(), std::slice::from_ref(&channel));

    // An update replaces the record whole: the old username and number stop finding it.
    let renamed = peer(100, &["alice2"], None, 30);
    engine.update_peers(std::slice::from_ref(&renamed)).await.unwrap();
    assert_eq!(engine.get_peer_by_username("alice").await.unwrap(), None);
    assert_eq!(engine.get_peer_by_phone_number("15550001").await.unwrap(), None);
    assert_eq!(engine.get_peer_by_username("alice2").await.unwrap(), Some(renamed.clone()));
    let taker = peer(200, &["alice2"], None, 40);
    engine.update_peers(std::slice::from_ref(&taker)).await.unwrap();
    assert_eq!(engine.get_peer_by_username("alice2").await.unwrap(), Some(taker.clone()), "a taken username moves");

    engine.close().await.unwrap();
    engine.open().await.unwrap();
    assert_eq!(engine.load_session().await.unwrap().map(|s| fields(&s)), Some(fields(&session)));
    assert_eq!(engine.load_state().await.unwrap(), Some(state.clone()));
    assert_eq!(engine.export_peers(None).await.unwrap(), [taker, renamed, channel], "close then open keeps all");

    engine.clear_peers().await.unwrap();
    assert!(engine.export_peers(None).await.unwrap().is_empty());
    assert_eq!(engine.get_peer_by_id(100).await.unwrap(), None);
    assert_eq!(engine.get_peer_by_username("news").await.unwrap(), None);
    assert!(engine.load_session().await.unwrap().is_some(), "clearing peers keeps the session");
    assert_eq!(engine.load_state().await.unwrap(), Some(state));

    dropped_key_passes_to_the_newest_remaining_holder(engine).await;

    engine.clear_peers().await.unwrap();
    engine.delete().await.unwrap();
    assert!(engine.load_session().await.unwrap().is_none());
    assert!(engine.load_state().await.unwrap().is_none());
    engine.save_session(&session).await.unwrap();
    assert!(engine.load_session().await.unwrap().is_some(), "a new login after delete is kept");
    engine.delete().await.unwrap();
    engine.close().await.unwrap();
}

/// A username or number belongs to the newest `(updated, id)` peer listing it, and passes to the newest remaining
/// one when its owner lets it go: no lookup misses while some peer still has it.
async fn dropped_key_passes_to_the_newest_remaining_holder(engine: &dyn Storage) {
    let older = peer(300, &["shared"], Some("15550300"), 50);
    let newer = peer(400, &["shared"], Some("15550300"), 60);
    let tied = peer(500, &["tied"], None, 70);
    let tied_larger = peer(600, &["tied"], None, 70);
    engine.update_peers(&[tied_larger.clone(), newer.clone(), older.clone(), tied]).await.unwrap();
    assert_eq!(engine.get_peer_by_username("shared").await.unwrap(), Some(newer));
    assert_eq!(engine.get_peer_by_phone_number("15550300").await.unwrap().map(|p| p.id), Some(400));
    assert_eq!(engine.get_peer_by_username("tied").await.unwrap(), Some(tied_larger), "a tie goes to the larger id");

    let dropped = peer(400, &[], None, 80);
    engine.update_peers(std::slice::from_ref(&dropped)).await.unwrap();
    assert_eq!(engine.get_peer_by_username("shared").await.unwrap(), Some(older.clone()), "the name passes on");
    assert_eq!(engine.get_peer_by_phone_number("15550300").await.unwrap(), Some(older), "the number passes on");
    assert_eq!(engine.get_peer_by_id(400).await.unwrap(), Some(dropped));
}
