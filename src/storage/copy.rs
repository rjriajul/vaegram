use std::io;

use super::Storage;

const BATCH: usize = 512;

/// Copies the session, the update state and every peer from one engine into another, to move a client to a new
/// engine or to import a session another library saved. `to` keeps whatever `from` doesn't have.
///
/// Both engines are opened and closed again; neither may be in use by a running client. The copy runs on the
/// caller's runtime, so an engine that blocks blocks it while the copy lasts.
pub async fn copy(from: &dyn Storage, to: &dyn Storage) -> io::Result<()> {
    from.open().await?;
    to.open().await?;
    if let Some(session) = from.load_session().await? {
        to.save_session(&session).await?;
    }
    if let Some(state) = from.load_state().await? {
        to.save_state(&state).await?;
    }
    for peers in from.export_peers(None).await?.chunks(BATCH) {
        to.update_peers(peers).await?;
    }
    to.close().await?;
    from.close().await
}

#[cfg(test)]
mod tests {
    use super::copy;
    use crate::session::Salt;
    use crate::storage::{FileStorage, MemoryStorage, PeerRecord, SessionData, Storage, StoredState, UpdateState};

    #[tokio::test]
    async fn everything_moves_to_the_new_engine() {
        let dir = std::env::temp_dir().join(format!("vaegram-copy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let from = MemoryStorage::new();
        let mut session = SessionData::new(4, [3; 256]);
        session.user_id = 42;
        from.save_session(&session).await.unwrap();
        let state = StoredState::new(
            42,
            Salt { value: 1, time_offset: 2 },
            Some(UpdateState { pts: 5, date: 9, ..UpdateState::default() }),
        );
        from.save_state(&state).await.unwrap();
        let mut peers: Vec<PeerRecord> = (1..=1200).map(|id| PeerRecord::new(id, id * 2)).collect();
        (peers[0].usernames, peers[0].updated) = (vec!["alice".into()], 10);
        from.update_peers(&peers).await.unwrap();

        let to = FileStorage::new(dir.join("bot"));
        copy(&from, &to).await.unwrap();
        let again = FileStorage::new(dir.join("bot"));
        let copied = (
            again.load_session().await.unwrap().map(|s| s.user_id),
            again.load_state().await.unwrap(),
            again.export_peers(None).await.unwrap().len(),
            again.get_peer_by_username("alice").await.unwrap().map(|p| p.id),
        );
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(copied, (Some(42), Some(state), 1200, Some(1)));
    }
}
