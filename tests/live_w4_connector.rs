use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vaegram::connection::Options;
use vaegram::raw::{base, functions};
use vaegram::session::Session;
use vaegram::session::internals::data_center;

#[tokio::test]
#[ignore = "connects to Telegram"]
async fn connector_dials_production() {
    let dc = data_center::find(&[], 2, false, false).unwrap();
    let dials = Arc::new(AtomicUsize::new(0));
    let counted = dials.clone();
    let factory = move |addr: SocketAddr| {
        counted.fetch_add(1, Ordering::Relaxed);
        tokio::net::TcpStream::connect(addr)
    };
    let options = Options::new().connection_factory(factory);
    let (session, _updates) = Session::connect(dc, None, &options, Arc::default(), None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 5 }).await.unwrap();
    assert_eq!((pong.ping_id, dials.load(Ordering::Relaxed)), (5, 1));
}
