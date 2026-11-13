//! The session task over WebSocket.

use tokio::net::TcpListener;

use super::{Arc, Options, Runner, Salt, mpsc};
use crate::connection::{open_plain_websocket, ping_when_idle};
use crate::session::internals::data_center::Dc;

#[tokio::test]
async fn an_idle_session_answers_a_websocket_ping() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dc = Dc { id: 2, addr: listener.local_addr().unwrap(), test: false, media: false };
    let server = tokio::spawn(ping_when_idle(listener));
    let options = Options::default();
    let conn = open_plain_websocket(&dc, &options).await;
    let (tx, _updates) = mpsc::channel(8);
    let runner = Runner::new(dc, Arc::new(options), conn, ([7; 256], Salt::default(), true), Arc::default(), Some(tx));
    let (_requests, requests) = mpsc::unbounded_channel();
    // Spawned as `Session` spawns it, which also holds the task to `Send`.
    let task = tokio::spawn(runner.run(requests));
    let (pong, _stream) = server.await.unwrap();
    task.abort();
    assert_eq!(pong.as_deref(), Some(&b"idle"[..]), "no pong within 1 s");
}
