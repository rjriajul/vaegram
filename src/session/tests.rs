use std::sync::Mutex;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{
    Affected, Answer, Arc, Call, ClientEvent, Connection, DEAD_AFTER, Identifiable, Incoming, MAX_CONTAINER_MSGS,
    Observer, Options, PING_EVERY, PROBE_AFTER, PROBE_DEAD_AFTER, RPC_RESULT, Request, Runner, Salt, Serializable,
    Session, UpdatesRx, base, constructor_id, functions, gzip_packed, internals, jitter, mpsc, msg_container, mtproto,
    oneshot, outbox, skips_backoff, types,
};
use crate::errors::{Error, RpcError};
use crate::session::internals::data_center::Dc;
use crate::session::test_dc::{self, TestDc};
use crate::session::transfer::{Auth, TransferPool};
use crate::session::{Known, Learned, auth::Created, pfs::Pfs};

type Frames = mpsc::UnboundedReceiver<Vec<u8>>;
type Seen = Arc<Mutex<Vec<(&'static str, i64, Result<Vec<u8>, String>)>>>;

async fn runner() -> (Runner, Frames, UpdatesRx) {
    runner_with(Salt::default()).await
}

async fn runner_with(salt: Salt) -> (Runner, Frames, UpdatesRx) {
    let (runner, frames, updates, _) = listening(salt).await;
    (runner, frames, updates)
}

async fn listening(salt: Salt) -> (Runner, Frames, UpdatesRx, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let options = Options::new().transport(crate::connection::Transport::Intermediate);
    let dc = Dc { id: 2, addr, test: false, media: false };
    let conn = Connection::connect(&dc, &options).await.unwrap();
    let (mut server, _) = listener.accept().await.unwrap();
    server.read_exact(&mut [0; 4]).await.unwrap();
    let (frames_tx, frames) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok(len) = server.read_u32_le().await {
            let mut frame = vec![0; len as usize];
            if server.read_exact(&mut frame).await.is_err() || frames_tx.send(frame).is_err() {
                break;
            }
        }
    });

    let (events, (tx, rx)) = (Arc::default(), mpsc::channel(Options::default().updates_capacity));
    let runner = Runner::new(dc, Arc::new(options), conn, ([7; 256], salt, true), Arc::clone(&events), Some(tx));
    let updates = UpdatesRx { rx, events };
    (runner, frames, updates, listener)
}

async fn recv(frames: &mut Frames, r: &Runner) -> (i64, i32, Vec<u8>) {
    let packet = frames.recv().await.unwrap();
    let m = mtproto::unpack(&packet, &r.auth_key, &r.auth_key_id, &r.session_id, false).unwrap();
    (m.msg_id, m.seq_no, m.body)
}

fn request(r: &mut Runner, body: Vec<u8>) -> oneshot::Receiver<Answer> {
    let (answer, rx) = oneshot::channel();
    r.enqueue(Request { name: "ping", body, tag: None, answer });
    rx
}

fn ping(ping_id: i64) -> Vec<u8> {
    functions::Ping { ping_id }.to_bytes()
}

fn observe(r: &mut Runner) -> Seen {
    let seen = Seen::default();
    let log = seen.clone();
    Arc::make_mut(&mut r.options).observer = Some(Observer(Arc::new(move |c: &Call<'_>| {
        let result = c.result.map(<[u8]>::to_vec).map_err(ToString::to_string);
        log.lock().unwrap().push((c.name, c.msg_id, result));
    })));
    seen
}

fn rpc_result(req_msg_id: i64, result: &[u8]) -> Vec<u8> {
    [&RPC_RESULT.to_le_bytes()[..], &req_msg_id.to_le_bytes(), result].concat()
}

#[tokio::test]
async fn packs_acks_and_requests_into_one_container() {
    let (mut r, mut frames, _updates) = runner().await;
    r.acks = vec![11, 13];
    let _rx: Vec<_> = (0..3).map(|i| request(&mut r, ping(i))).collect();
    r.flush().await.unwrap();

    let (container, seq_no, body) = recv(&mut frames, &r).await;
    let msgs = msg_container::parse(&body).unwrap();
    assert_eq!((seq_no % 2, msgs.len(), msgs[0].seq_no % 2), (0, 4, 0));
    assert_eq!(msgs[0].body, types::MsgsAck { msg_ids: vec![11, 13] }.to_bytes());
    for (i, m) in msgs[1..].iter().enumerate() {
        assert_eq!((m.body, m.seq_no % 2, r.pending[&m.msg_id].container), (&ping(i as i64)[..], 1, container));
    }
    assert!(msgs.windows(2).all(|w| w[0].msg_id < w[1].msg_id) && msgs[3].msg_id < container);
    assert!(r.acks.is_empty() && r.queue.is_empty());
}

#[tokio::test]
async fn splits_at_the_message_and_size_caps() {
    let (mut r, mut frames, _updates) = runner().await;
    let _rx: Vec<_> = (0..=MAX_CONTAINER_MSGS as i64).map(|i| request(&mut r, ping(i))).collect();
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(msg_container::parse(&body).unwrap().len(), MAX_CONTAINER_MSGS);
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(body, ping(MAX_CONTAINER_MSGS as i64));

    let big = vec![0; outbox::MAX_CONTAINER_BYTES / 2 - 16];
    let _rx: Vec<_> = (0..3).map(|_| request(&mut r, big.clone())).collect();
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(msg_container::parse(&body).unwrap().len(), 2);
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(body, big);
    assert_eq!(r.pending.len(), MAX_CONTAINER_MSGS + 4);
}

#[tokio::test]
async fn keeps_each_get_file_in_a_container_of_its_own() {
    let (mut r, mut frames, _updates) = runner().await;
    let location =
        types::InputDocumentFileLocation { id: 1, access_hash: 2, file_reference: vec![3], thumb_size: String::new() };
    let location = base::InputFileLocation::InputDocumentFileLocation(Box::new(location));
    let get_file = |offset| functions::upload::GetFile {
        precise: false,
        cdn_supported: false,
        location: location.clone(),
        offset,
        limit: 512 << 10,
    };
    let in_takeout = functions::InvokeWithTakeout { takeout_id: 9, query: get_file(1 << 19) }.to_bytes();
    let (first, last) = (get_file(0).to_bytes(), get_file(2 << 19).to_bytes());
    let bodies = [first.clone(), in_takeout.clone(), ping(1), last.clone()];
    let _rx: Vec<_> = bodies.into_iter().map(|body| request(&mut r, body)).collect();
    r.flush().await.unwrap();

    assert_eq!(recv(&mut frames, &r).await.2, first);
    let (_, _, body) = recv(&mut frames, &r).await;
    let packed: Vec<_> = msg_container::parse(&body).unwrap().into_iter().map(|m| m.body.to_vec()).collect();
    assert_eq!(packed, [in_takeout, ping(1)], "another request may share a getFile's container");
    assert_eq!(recv(&mut frames, &r).await.2, last);
}

#[tokio::test]
async fn forgets_abandoned_requests() {
    let (mut r, mut frames, _updates) = runner().await;
    drop(request(&mut r, ping(1)));
    drop(request(&mut r, ping(2)));
    let mut live = request(&mut r, ping(3));
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let ids: Vec<i64> = msg_container::parse(&body).unwrap().iter().map(|m| m.msg_id).collect();

    let (answer, _) = oneshot::channel();
    r.enqueue(Request { name: "ping", body: ping(4), tag: None, answer });
    assert!(r.queue.is_empty());
    r.drop_abandoned();
    assert!(live.try_recv().is_err());
    assert_eq!(r.pending.keys().collect::<Vec<_>>(), [&ids[2]]);
    let mut drops: Vec<_> = r.service.iter().map(|(b, _)| b.clone()).collect();
    drops.sort();
    let mut expected = [ids[0], ids[1]].map(|req_msg_id| functions::RpcDropAnswer { req_msg_id }.to_bytes());
    expected.sort();
    assert_eq!(drops, expected);

    r.resend_pending().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(body, types::MsgsStateReq { msg_ids: vec![ids[2]] }.to_bytes());
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(msg_container::parse(&body).unwrap().len(), 3);
    assert_eq!(r.pending.values().map(|p| &p.body).collect::<Vec<_>>(), [&ping(3)]);
}

async fn reconnect_with_one_unacked(r: &mut Runner, frames: &mut Frames) -> (i64, i64, Vec<oneshot::Receiver<Answer>>) {
    let rx = (0..2).map(|i| request(r, ping(i))).collect();
    r.flush().await.unwrap();
    let (_, _, body) = recv(frames, r).await;
    let ids: Vec<i64> = msg_container::parse(&body).unwrap().iter().map(|m| m.msg_id).collect();

    r.handle_message(1, 0, &types::MsgsAck { msg_ids: vec![ids[0]] }.to_bytes());
    r.resend_pending().await.unwrap();
    let (check, seq_no, body) = recv(frames, r).await;
    assert_eq!(body, types::MsgsStateReq { msg_ids: vec![ids[1]] }.to_bytes());
    assert_eq!(seq_no % 2, 1);
    let (_, _, body) = recv(frames, r).await;
    assert_eq!(constructor_id(&body), Some(<functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID));
    assert!(r.pending.contains_key(&ids[0]) && r.pending.contains_key(&ids[1]) && r.queue.is_empty());
    (ids[1], check, rx)
}

#[tokio::test]
async fn reconnect_resends_only_what_the_server_never_got() {
    let (mut r, mut frames, _updates) = runner().await;
    let (unacked, check, _rx) = reconnect_with_one_unacked(&mut r, &mut frames).await;
    r.handle_message(3, 0, &types::MsgsStateInfo { req_msg_id: check, info: vec![2] }.to_bytes());
    r.flush().await.unwrap();
    let (msg_id, _, body) = recv(&mut frames, &r).await;
    assert_eq!(body, ping(1));
    assert!(msg_id > unacked && !r.pending.contains_key(&unacked) && r.pending.contains_key(&msg_id));
    assert!(r.checking.is_empty());
}

#[tokio::test]
async fn reconnect_keeps_waiting_for_what_the_server_got() {
    let (mut r, mut frames, _updates) = runner().await;
    let (unacked, check, _rx) = reconnect_with_one_unacked(&mut r, &mut frames).await;
    r.handle_message(3, 0, &types::MsgsStateInfo { req_msg_id: check, info: vec![1 | 32] }.to_bytes());
    assert!(r.pending[&unacked].acked && r.queue.is_empty() && r.checking.is_empty());

    r.resend_pending().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    assert_eq!(constructor_id(&body), Some(<functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID));
}

#[tokio::test]
async fn a_status_with_the_received_flag_is_not_resent() {
    let (mut r, mut frames, _updates) = runner().await;
    let (unacked, check, _rx) = reconnect_with_one_unacked(&mut r, &mut frames).await;
    r.handle_message(3, 0, &types::MsgsStateInfo { req_msg_id: check, info: vec![2 | 128] }.to_bytes());
    assert!(r.queue.is_empty() && r.pending[&unacked].acked);
}

#[tokio::test]
async fn ids_a_short_status_answer_leaves_out_are_asked_again() {
    let (mut r, mut frames, _updates) = runner().await;
    let _rx: Vec<_> = (0..2).map(|i| request(&mut r, ping(i))).collect();
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    r.resend_pending().await.unwrap();
    let (check, _, body) = recv(&mut frames, &r).await;
    let asked = <base::MsgsStateReq as crate::raw::Deserializable>::from_bytes(&body).unwrap();
    let base::MsgsStateReq::MsgsStateReq(asked) = asked;
    let ids = asked.msg_ids;
    assert_eq!(ids.len(), 2);

    r.handle_message(3, 0, &types::MsgsStateInfo { req_msg_id: check, info: vec![4] }.to_bytes());
    assert!(r.pending[&ids[0]].acked && !r.pending[&ids[1]].acked && r.queue.is_empty());
    assert_eq!(r.recheck, [ids[1]]);
}

#[tokio::test]
async fn a_status_check_refused_for_another_reason_is_asked_again() {
    let (mut r, mut frames, _updates) = runner().await;
    let (unacked, check, _rx) = reconnect_with_one_unacked(&mut r, &mut frames).await;
    let bad = types::BadMsgNotification { bad_msg_id: check, bad_msg_seqno: 1, error_code: 64 };
    r.handle_message(3, 0, &bad.to_bytes());
    assert!(r.checking.is_empty() && r.queue.is_empty() && r.pending.contains_key(&unacked));
    assert_eq!(r.recheck, [unacked]);
}

#[tokio::test]
async fn status_check_refused_for_bad_salt_is_asked_again() {
    let (mut r, mut frames, _updates) = runner().await;
    let (unacked, check, _rx) = reconnect_with_one_unacked(&mut r, &mut frames).await;
    let salt = types::BadServerSalt { bad_msg_id: check, bad_msg_seqno: 1, error_code: 48, new_server_salt: 5 };
    r.handle_message(3, 0, &salt.to_bytes());
    assert_eq!(r.recheck, [unacked]);
    r.check_state().await.unwrap();
    let (again, _, body) = recv(&mut frames, &r).await;
    assert_eq!(body, types::MsgsStateReq { msg_ids: vec![unacked] }.to_bytes());
    assert!(r.checking.contains_key(&again) && !r.checking.contains_key(&check));
    assert!(r.queue.is_empty() && r.pending.contains_key(&unacked));
}

#[tokio::test]
async fn silent_connection_is_declared_dead() {
    let (mut r, _frames, _updates) = runner().await;
    // Paused only after connecting: a paused clock would jump past the connect timeout.
    tokio::time::pause();
    assert!(r.tick().await.is_ok());
    tokio::time::advance(DEAD_AFTER + Duration::from_secs(1)).await;
    assert!(matches!(r.tick().await, Err(Error::Disconnected)));
}

#[tokio::test]
async fn silence_with_requests_waiting_is_probed_once() {
    const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
    let (mut r, mut frames, _updates) = runner().await;
    let mut waiting = vec![request(&mut r, ping(1))];
    r.flush().await.unwrap();
    assert_eq!(recv(&mut frames, &r).await.2, ping(1));

    tokio::time::pause();
    tokio::time::advance(PROBE_AFTER + Duration::from_secs(1)).await;
    for next in [2, 3] {
        r.tick().await.unwrap();
        waiting.push(request(&mut r, ping(next)));
        r.flush().await.unwrap();
        if next == 2 {
            assert_eq!(constructor_id(&recv(&mut frames, &r).await.2), Some(PING_DELAY_DISCONNECT));
        }
        assert_eq!(recv(&mut frames, &r).await.2, ping(next));
    }
}

#[tokio::test]
async fn a_read_after_a_probe_lets_the_next_silence_be_probed() {
    const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
    let (mut r, mut frames, _updates) = runner().await;
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    tokio::time::pause();
    for _ in 0..2 {
        tokio::time::advance(PROBE_AFTER + Duration::from_secs(1)).await;
        r.tick().await.unwrap();
        assert_eq!(constructor_id(&recv(&mut frames, &r).await.2), Some(PING_DELAY_DISCONNECT));
        r.heard();
    }
}

#[tokio::test]
async fn silence_with_requests_waiting_is_declared_dead_sooner() {
    let (mut r, _frames, _updates) = runner().await;
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(PROBE_DEAD_AFTER + Duration::from_secs(1)).await;
    assert!(matches!(r.tick().await, Err(Error::Disconnected)));
}

#[tokio::test]
async fn idle_silence_is_not_probed() {
    let (mut r, mut frames, _updates) = runner().await;
    tokio::time::pause();
    tokio::time::advance(PING_EVERY - Duration::from_secs(1)).await;
    r.tick().await.unwrap();
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    assert_eq!(recv(&mut frames, &r).await.2, ping(1));
}

/// A transfer connection's runner, with the server's end of it to write to.
async fn transfer_runner() -> (Runner, tokio::net::TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let options = Options::new().transport(crate::connection::Transport::Intermediate);
    let dc = Dc { id: 2, addr: listener.local_addr().unwrap(), test: false, media: false };
    let conn = Connection::connect(&dc, &options).await.unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let runner = Runner::new(dc, Arc::new(options), conn, ([7; 256], Salt::default(), true), Arc::default(), None);
    (runner, server)
}

#[tokio::test]
async fn a_silent_transfer_connection_with_a_part_waiting_is_declared_dead() {
    let (mut r, _server) = transfer_runner().await;
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(PROBE_AFTER + Duration::from_secs(1)).await;
    r.tick().await.unwrap();
    assert!(r.probed);
    // The probe went out just now, and the server would have answered it by this.
    tokio::time::advance(PROBE_DEAD_AFTER + Duration::from_secs(1)).await;
    assert!(matches!(r.tick().await, Err(Error::Disconnected)));
}

#[tokio::test]
async fn a_transfer_connection_still_reading_a_big_part_is_alive() {
    let (mut r, mut server) = transfer_runner().await;
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(PROBE_DEAD_AFTER).await;
    // The start of a 1 MiB packet, the rest still on its way.
    server.write_all(&(1u32 << 20).to_le_bytes()).await.unwrap();
    server.write_all(&[0; 1024]).await.unwrap();
    // Read until those bytes are in, however long the socket takes to show them; the packet stays unfinished.
    let before = r.reader.last_byte();
    while r.reader.last_byte() == before {
        let _ = tokio::time::timeout(Duration::from_millis(1), r.reader.recv()).await;
    }
    tokio::time::advance(PROBE_AFTER - Duration::from_secs(1)).await;
    r.tick().await.unwrap();
    assert!(!r.probed, "a connection still reading was probed");
}

#[tokio::test]
async fn a_transfer_connection_just_done_writing_a_slow_part_is_alive() {
    let (mut r, _server) = transfer_runner().await;
    let _first = request(&mut r, ping(1));
    r.flush().await.unwrap();
    tokio::time::pause();
    // As if the next part took this long to go out: the server can't have answered it yet.
    tokio::time::advance(PROBE_DEAD_AFTER + Duration::from_secs(1)).await;
    let _next = request(&mut r, ping(2));
    r.flush().await.unwrap();
    r.tick().await.unwrap();
    assert!(!r.probed, "a connection that just wrote was probed");
}

#[tokio::test]
async fn an_idle_transfer_connection_is_not_watched() {
    let (mut r, _server) = transfer_runner().await;
    tokio::time::pause();
    tokio::time::advance(DEAD_AFTER + Duration::from_secs(30)).await;
    r.tick().await.unwrap();
    assert!(!r.probed);
}

#[tokio::test]
async fn transfer_connections_wait_out_slow_parts() {
    // Two 512 KiB parts, as an upload connection carries: at MIN_RATE they reach the server 128 s after they're
    // written, and it can't answer either before that.
    let (mut r, _frames, _updates) = runner().await;
    r.updates = None;
    let _rx: Vec<_> = (0..2).map(|_| request(&mut r, vec![0; 512 << 10])).collect();
    r.flush().await.unwrap();
    tokio::time::pause();
    tokio::time::advance(DEAD_AFTER + Duration::from_secs(30)).await;
    assert!(r.tick().await.is_ok());
    assert!(r.service.is_empty(), "a transfer connection was probed");
}

#[tokio::test]
async fn a_transfer_connection_is_probed_once_its_part_could_have_arrived() {
    let (mut r, _frames, _updates) = runner().await;
    r.updates = None;
    let _rx = request(&mut r, vec![0; 512 << 10]);
    r.flush().await.unwrap();
    tokio::time::pause();
    // 512 KiB reach the server 64 s after they're written, at MIN_RATE.
    tokio::time::advance(Duration::from_secs(64) + PROBE_AFTER - Duration::from_secs(1)).await;
    r.tick().await.unwrap();
    assert!(!r.probed, "probed before the part could have arrived");
    tokio::time::advance(Duration::from_secs(2)).await;
    r.tick().await.unwrap();
    assert!(r.probed);
}

#[test]
fn reconnect_delay_is_jittered_within_the_upper_half() {
    let delays: Vec<Duration> = (0..64).map(|_| jitter(Duration::from_secs(8))).collect();
    assert!(delays.iter().all(|d| (Duration::from_secs(4)..=Duration::from_secs(8)).contains(d)));
    assert!(delays.windows(2).any(|w| w[0] != w[1]));
}

#[tokio::test]
async fn bad_seqno_resyncs_and_resends() {
    for code in [32, 33] {
        let (mut r, mut frames, _updates) = runner().await;
        let mut rx = request(&mut r, ping(1));
        r.flush().await.unwrap();
        let (bad_msg_id, bad_seq, _) = recv(&mut frames, &r).await;

        let notification = types::BadMsgNotification { bad_msg_id, bad_msg_seqno: bad_seq, error_code: code };
        r.handle_message(1, 0, &notification.to_bytes());
        r.flush().await.unwrap();
        let (msg_id, seq_no, body) = recv(&mut frames, &r).await;
        assert_eq!(
            (body, msg_id > bad_msg_id, seq_no > bad_seq, r.pending.contains_key(&msg_id)),
            (ping(1), true, code == 32, true)
        );
        assert!(rx.try_recv().is_err());
    }
}

#[tokio::test]
async fn bad_salt_for_a_container_resends_every_request() {
    let (mut r, mut frames, _updates) = runner().await;
    let _rx: Vec<_> = (0..2).map(|i| request(&mut r, ping(i))).collect();
    r.flush().await.unwrap();
    let (bad_msg_id, bad_msg_seqno, _) = recv(&mut frames, &r).await;

    let salt = types::BadServerSalt { bad_msg_id, bad_msg_seqno, error_code: 48, new_server_salt: 1 };
    r.handle_message(1, 0, &salt.to_bytes());
    assert_eq!((r.salt(), r.queue.len(), r.pending.len()), (1, 2, 0));
}

#[tokio::test]
async fn learned_salt_and_clock_are_kept_for_the_state_file() {
    let (mut r, mut frames, _updates) = runner().await;
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let (bad_msg_id, bad_msg_seqno, _) = recv(&mut frames, &r).await;

    let salt = types::BadServerSalt { bad_msg_id, bad_msg_seqno, error_code: 48, new_server_salt: 77 };
    r.handle_message(1, 0, &salt.to_bytes());
    let server_time = internals::now() + 100;
    let too_low = types::BadMsgNotification { bad_msg_id, bad_msg_seqno, error_code: 16 };
    r.handle_message((server_time << 32) | 1, 0, &too_low.to_bytes());
    let session = Session { learned: r.learned.clone(), ..Session::local(mpsc::unbounded_channel().0) };
    let saved = session.salt();
    assert_eq!(saved.value, 77);
    assert!((saved.time_offset - 100).abs() <= 1, "{saved:?}");
}

#[tokio::test]
async fn starts_with_the_saved_salt_and_clock() {
    let (mut r, _frames, _updates) = runner_with(Salt { value: 9, time_offset: -40 }).await;
    assert_eq!(r.salt(), 9);
    assert!(((r.msg_ids.next() >> 32) - internals::now() + 40).abs() <= 1);
}

#[tokio::test]
async fn answers_msgs_state_req() {
    let (mut r, mut frames, _updates) = runner().await;
    r.acks = vec![5];
    r.handle_message(101, 2, &types::MsgsStateReq { msg_ids: vec![5, 9] }.to_bytes());
    r.flush().await.unwrap();

    let (_, _, body) = recv(&mut frames, &r).await;
    let msgs = msg_container::parse(&body).unwrap();
    assert_eq!(msgs[1].body, types::MsgsStateInfo { req_msg_id: 101, info: vec![4, 1] }.to_bytes());
    assert_eq!(msgs[1].seq_no % 2, 0);
}

// An answer we took in and whose ack has gone out, so `acks` no longer holds it.
async fn received_and_acked(r: &mut Runner) -> i64 {
    let msg_id = (internals::now() << 32) | 1;
    assert!(r.fresh(msg_id, &ping(1)));
    r.acks.push(msg_id);
    r.flush().await.unwrap();
    assert!(r.acks.is_empty());
    msg_id
}

#[tokio::test]
async fn new_detailed_info_for_a_flushed_answer_acks_instead_of_resending() {
    let (mut r, _frames, _updates) = runner().await;
    let answer = received_and_acked(&mut r).await;
    let info = types::MsgNewDetailedInfo { answer_msg_id: answer, bytes: 8, status: 0 };
    r.handle_message(answer + 4, 0, &info.to_bytes());
    assert_eq!(r.acks, vec![answer]);
    assert!(r.service.is_empty(), "an answer we have was asked for again");

    let missing = types::MsgNewDetailedInfo { answer_msg_id: answer + 16, bytes: 8, status: 0 };
    r.handle_message(answer + 12, 0, &missing.to_bytes());
    assert_eq!(r.service.back().unwrap().0, types::MsgResendReq { msg_ids: vec![answer + 16] }.to_bytes());
}

#[tokio::test]
async fn new_detailed_info_for_a_queued_ack_adds_no_duplicate() {
    let (mut r, _frames, _updates) = runner().await;
    let answer = received_and_acked(&mut r).await;
    let info = types::MsgNewDetailedInfo { answer_msg_id: answer, bytes: 8, status: 0 }.to_bytes();
    r.handle_message(answer + 4, 0, &info);
    r.handle_message(answer + 8, 0, &info);
    assert_eq!(r.acks, vec![answer]);
    assert!(r.service.is_empty());
}

#[tokio::test]
async fn state_req_reports_4_for_an_acked_message() {
    let (mut r, _frames, _updates) = runner().await;
    let got = received_and_acked(&mut r).await;
    r.handle_message(got + 4, 0, &types::MsgsStateReq { msg_ids: vec![got, got + 8] }.to_bytes());
    let info = types::MsgsStateInfo { req_msg_id: got + 4, info: vec![4, 1] }.to_bytes();
    assert_eq!(r.service.back().unwrap().0, info);
}

#[tokio::test]
async fn dropped_updates_ask_for_a_difference() {
    let (mut r, _frames, mut updates) = runner().await;
    let events = r.events.record();
    let empty = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
    let body = base::Updates::Updates(Box::new(empty)).to_bytes();
    for id in 0..=Options::default().updates_capacity as i64 {
        r.handle_message(id, 0, &body);
    }
    let first = updates.recv().await.unwrap().updates;
    let second = updates.recv().await.unwrap().updates;
    assert!(matches!((first, second), (base::Updates::UpdatesTooLong, base::Updates::Updates(_))));
    assert!(matches!(events.try_recv(), Ok(ClientEvent::UpdatesDropped { count: 1, unrecoverable: 0 })));
}

#[tokio::test]
async fn affected_pts_come_back_as_quiet_updates() {
    let mut updates = UpdatesRx::offline();
    updates.events.affected(Affected { channel_id: Some(7), pts: 5, pts_count: 1 });
    let incoming = updates.recv().await.unwrap();
    let from_request = incoming.from_request;
    let base::Updates::UpdateShort(short) = incoming.updates else { panic!("not a stand-in update") };
    let channel = matches!(&short.update, base::Update::UpdateDeleteChannelMessages(u) if u.channel_id == 7);
    assert_eq!((from_request, channel, short.update.pts()), (true, true, Some((5, 1))));
}

#[tokio::test]
async fn affected_pts_wake_a_waiting_receiver() {
    // The update stream stays open and quiet: only the affected counter can end the wait.
    let (_stream, mut updates) = UpdatesRx::channel();
    let events = updates.events.clone();
    let waiting = tokio::spawn(async move { updates.recv().await.map(|incoming| incoming.from_request) });
    tokio::task::yield_now().await;
    events.affected(Affected { channel_id: None, pts: 9, pts_count: 2 });
    let got = tokio::time::timeout(Duration::from_secs(5), waiting).await;
    assert!(matches!(got, Ok(Ok(Some(true)))), "the receiver slept through it: {got:?}");
}

#[tokio::test]
async fn drops_replayed_and_stale_server_messages() {
    let (mut r, _frames, _updates) = runner().await;
    let now = crate::session::internals::now();
    let update = ping(1);
    let id = (now << 32) | 1;
    assert!(r.fresh(id, &update));
    assert!(!r.fresh(id, &update));
    assert!(!r.fresh(id + 3, &update), "even ids are client ids");
    assert!(!r.fresh(((now - 600) << 32) | 1, &update));

    let sync = types::BadMsgNotification { bad_msg_id: 4, bad_msg_seqno: 0, error_code: 16 }.to_bytes();
    assert!(r.fresh(((now + 3600) << 32) | 1, &sync));
}

#[tokio::test]
async fn stored_key_waits_for_a_measured_clock_before_the_time_window() {
    let (mut r, mut frames, _updates) = runner().await;
    r.clock_synced = false;
    let skewed = crate::session::internals::now() + 3600;
    assert!(r.fresh((skewed << 32) | 1, &ping(1)));
    assert!(!r.fresh((skewed << 32) | 1, &ping(1)), "duplicates are still dropped");

    let mut rx = request(&mut r, ping(2));
    r.flush().await.unwrap();
    let (bad_msg_id, bad_msg_seqno, _) = recv(&mut frames, &r).await;
    let notification = types::BadMsgNotification { bad_msg_id, bad_msg_seqno, error_code: 16 };
    r.handle_message((skewed << 32) | 5, 0, &notification.to_bytes());
    assert!(r.clock_synced);
    let session = Session { learned: r.learned.clone(), ..Session::local(mpsc::unbounded_channel().0) };
    assert!((session.salt().time_offset - 3600).abs() <= 1, "the corrected offset is saved");
    assert!(r.fresh((skewed << 32) | 9, &ping(1)));
    assert!(!r.fresh(((skewed - 600) << 32) | 1, &ping(1)));
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn stored_key_with_a_right_clock_turns_the_time_window_on() {
    let (mut r, _frames, _updates) = runner().await;
    r.clock_synced = false;
    let now = crate::session::internals::now();
    assert!(r.fresh((now << 32) | 1, &ping(1)));
    assert!(r.clock_synced);
    assert!(!r.fresh(((now - 600) << 32) | 1, &ping(1)));
}

// A packet the server sends `r`.
fn from_server(r: &Runner, msg_id: i64, body: &[u8]) -> Vec<u8> {
    super::replay::seal(&r.auth_key, &r.auth_key_id, &r.session_id, msg_id, 0, body)
}

// A msg_container of content-related messages with these ids, each carrying `body`.
fn container(ids: &[i64], body: &[u8]) -> Vec<u8> {
    let mut buf = [msg_container::ID.to_le_bytes(), (ids.len() as u32).to_le_bytes()].concat();
    for id in ids {
        buf.extend_from_slice(&id.to_le_bytes());
        buf.extend_from_slice(&1i32.to_le_bytes());
        buf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        buf.extend_from_slice(body);
    }
    buf
}

// Whether `got` is `n` drops by the replay check, and nothing else.
fn replays(got: &[ClientEvent], n: usize) -> bool {
    const REPLAYED: &str = "msg_id seen before, even or outside the time window";
    let replay = |e: &ClientEvent| matches!(e, ClientEvent::PacketDropped { reason } if *reason == REPLAYED);
    got.len() == n && got.iter().all(replay)
}

#[tokio::test]
async fn a_container_with_an_inner_id_not_below_its_own_is_dropped() {
    let (mut r, _frames, _updates) = runner().await;
    let events = r.events.record();
    let update = base::Updates::UpdatesTooLong.to_bytes();
    let outer = (internals::now() << 32) | 9;
    for newer in [outer, outer + 4] {
        let packet = from_server(&r, outer, &container(&[outer - 8, newer], &update));
        r.handle_packet(packet).await.unwrap();
        assert!(r.acks.is_empty(), "a message of the container was handled");
        let Ok(ClientEvent::PacketDropped { reason }) = events.try_recv() else { panic!("the drop wasn't reported") };
        assert_eq!(reason, "msg_container holds a newer msg_id");
    }
    assert!(r.fresh(outer, &update), "the dropped container's id was taken in");
}

// Fills the id set with ids newer than any a test container carries, so one more id forgets the oldest of them.
fn fill_id_set(r: &mut Runner, now: i64) {
    r.server_msg_ids.record((0..internals::ServerMsgIds::REMEMBERED as i64).map(|i| (now << 32) + 4 * i + 1));
}

#[tokio::test]
async fn a_full_id_set_still_accepts_a_containers_messages() {
    let (mut r, _frames, _updates) = runner().await;
    let now = internals::now();
    fill_id_set(&mut r, now);
    let (first, second, outer) = (((now - 10) << 32) | 1, ((now - 10) << 32) | 5, ((now + 1) << 32) | 1);
    let body = container(&[first, second], &base::Updates::UpdatesTooLong.to_bytes());
    let packet = from_server(&r, outer, &body);
    r.handle_packet(packet).await.unwrap();
    assert_eq!(r.acks, vec![first, second]);
    assert!(!r.fresh(outer, &ping(1)), "the container's own id was not taken in");
}

#[tokio::test]
async fn a_full_id_set_accepts_a_containers_messages_in_any_order() {
    let (mut r, _frames, _updates) = runner().await;
    let now = internals::now();
    fill_id_set(&mut r, now);
    let (first, second, outer) = (((now - 10) << 32) | 1, ((now - 10) << 32) | 5, ((now + 1) << 32) | 1);
    let seen = (now << 32) | 1;
    let body = container(&[second, seen, first], &base::Updates::UpdatesTooLong.to_bytes());
    let packet = from_server(&r, outer, &body);
    r.handle_packet(packet).await.unwrap();
    assert_eq!(r.acks, vec![second, first], "handled in wire order, and an id seen before is still dropped");
    assert!(!r.fresh(first, &ping(1)) && !r.fresh(second, &ping(1)) && !r.fresh(outer, &ping(1)));
}

#[tokio::test]
async fn a_msg_id_twice_in_one_container_is_delivered_once() {
    let (mut r, _frames, mut updates) = runner().await;
    let now = internals::now();
    let (inner, outer) = ((now << 32) | 1, (now << 32) | 9);
    let body = container(&[inner, inner], &base::Updates::UpdatesTooLong.to_bytes());
    let packet = from_server(&r, outer, &body);
    r.handle_packet(packet).await.unwrap();
    assert_eq!(r.acks, vec![inner]);
    assert!(matches!(updates.rx.try_recv(), Ok(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
    assert!(updates.rx.try_recv().is_err(), "the repeated message was delivered too");
}

#[tokio::test]
async fn a_container_with_an_even_or_stale_outer_id_delivers_nothing() {
    let (mut r, _frames, _updates) = runner().await;
    let events = r.events.record();
    let now = internals::now();
    let update = base::Updates::UpdatesTooLong.to_bytes();
    let (inner, replayed) = ((now << 32) | 1, (now << 32) | 9);
    assert!(r.fresh(replayed, &update));
    // Even, already seen, and too old (the last is older than its message: the id check comes first).
    for outer in [(now << 32) | 8, replayed, ((now - 600) << 32) | 1] {
        let packet = from_server(&r, outer, &container(&[inner], &update));
        r.handle_packet(packet).await.unwrap();
    }
    assert!(r.acks.is_empty(), "a message under a bad container id was handled");
    let got: Vec<ClientEvent> = events.try_iter().collect();
    assert!(replays(&got, 3), "each bad container id is reported: {got:?}");
    assert!(r.fresh(inner, &update), "a message under a bad container id was taken in");
}

#[tokio::test]
async fn a_replayed_msg_id_is_reported_and_still_dropped() {
    let (mut r, _frames, mut updates) = runner().await;
    let events = r.events.record();
    let now = internals::now();
    let update = base::Updates::UpdatesTooLong.to_bytes();
    let id = (now << 32) | 1;
    for _ in 0..2 {
        // Content-related (odd seq_no) like an update, so the first copy is owed an ack; `from_server` sends seq_no 0.
        let packet = super::replay::seal(&r.auth_key, &r.auth_key_id, &r.session_id, id, 1, &update);
        r.handle_packet(packet).await.unwrap();
    }
    // Inside a container too.
    let packet = from_server(&r, (now << 32) | 9, &container(&[id], &update));
    r.handle_packet(packet).await.unwrap();
    assert!(matches!(updates.rx.try_recv(), Ok(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
    assert!(updates.rx.try_recv().is_err(), "a replay was delivered");
    assert_eq!(r.acks, vec![id], "the first copy is acknowledged once, a replay not again");
    let got: Vec<ClientEvent> = events.try_iter().collect();
    assert!(replays(&got, 2), "{got:?}");
}

#[tokio::test]
async fn unknown_constructors_are_reported_and_skipped() {
    const UPDATE_SHORT: u32 = <types::UpdateShort as Identifiable>::CONSTRUCTOR_ID;
    let (mut r, mut frames, mut updates) = runner().await;
    let events = r.events.record();
    let unknown = 0x0bad_0badu32.to_le_bytes();
    r.handle_message(1, 1, &unknown);
    r.handle_message(3, 1, &[UPDATE_SHORT.to_le_bytes(), unknown, [0; 4]].concat());
    r.handle_message(5, 1, &UPDATE_SHORT.to_le_bytes());
    r.handle_message(7, 1, &gzip_packed::ID.to_le_bytes());
    assert!(r.handle_packet(vec![0; 64]).await.is_ok());
    let got: Vec<ClientEvent> = events.try_iter().collect();
    let expected = matches!(
        got[..],
        [
            ClientEvent::UnknownConstructor { id: 0x0bad_0bad },
            ClientEvent::UnknownConstructor { id: 0x0bad_0bad },
            ClientEvent::UpdateParseFailed { constructor_id: UPDATE_SHORT, len: 4 },
            ClientEvent::PacketDropped { reason: "bad gzip_packed" },
            ClientEvent::PacketDropped { reason: "failed decryption or the MTProto checks" },
        ]
    );
    assert!(expected, "{got:?}");

    r.handle_message(9, 1, &base::Updates::UpdatesTooLong.to_bytes());
    assert!(matches!(updates.recv().await, Some(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let msgs = msg_container::parse(&body).unwrap();
    assert_eq!(msgs[0].body, types::MsgsAck { msg_ids: vec![1, 3, 5, 7, 9] }.to_bytes());
    assert_eq!(msgs[1].body, ping(1));
}

#[tokio::test]
async fn answers_that_are_not_updates_are_not_reported() {
    let (r, _frames, _updates) = runner().await;
    let events = r.events.record();
    r.deliver(&ping(1), true);
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn reconnect_is_reported() {
    let (mut r, _frames, mut updates, listener) = listening(Salt::default()).await;
    let events = r.events.record();
    let server = tokio::spawn(async move { listener.accept().await.unwrap().0 });
    let (_sender, requests) = mpsc::unbounded_channel();
    r.reconnect(&requests, false, false).await;
    let _server = server.await.unwrap();
    assert!(matches!(events.try_recv(), Ok(ClientEvent::Reconnected { dc: 2, attempt: 1 })));
    assert!(r.events.take_reconnected());
    assert!(matches!(updates.recv().await, Some(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
}

#[tokio::test]
async fn restart_reconnects_and_is_reported() {
    let (mut r, _frames, mut updates, listener) = listening(Salt::default()).await;
    let events = r.events.record();
    let server = tokio::spawn(async move { listener.accept().await.unwrap().0 });
    let (_sender, requests) = mpsc::unbounded_channel();
    r.reconnect(&requests, false, true).await;
    let _server = server.await.unwrap();
    assert!(matches!(events.try_recv(), Ok(ClientEvent::Reconnected { dc: 2, attempt: 1 })));
    assert!(matches!(updates.recv().await, Some(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
}

#[tokio::test]
async fn close_ends_the_runner_and_fails_waiting_requests() {
    let (r, mut frames, mut updates) = runner().await;
    let control = r.control.clone();
    let (requests, rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(r.run(rx));
    let (answer, waiting) = oneshot::channel();
    requests.send(Request { name: "ping", body: ping(1), tag: None, answer }).unwrap();
    frames.recv().await.unwrap();
    control.close();
    tokio::time::timeout(Duration::from_secs(5), task).await.expect("the runner kept going").unwrap();
    assert!(matches!(waiting.await, Ok(Err(Error::Disconnected))));
    assert!(updates.recv().await.is_none());
}

#[tokio::test]
async fn close_during_backoff_ends_the_reconnect_at_once() {
    let (mut r, _frames, _updates, listener) = listening(Salt::default()).await;
    drop(listener);
    // Paused only after connecting; nothing connects again before the close.
    tokio::time::pause();
    let start = tokio::time::Instant::now();
    let control = r.control.clone();
    let (_sender, requests) = mpsc::unbounded_channel();
    let events = r.events.record();
    let closer = async {
        tokio::task::yield_now().await;
        control.close();
    };
    let reconnect = r.reconnect(&requests, false, false);
    let done = tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(reconnect, closer) }).await;
    assert!(done.is_ok(), "the reconnect kept backing off");
    assert_eq!(start.elapsed(), Duration::ZERO, "the backoff sleep wasn't cut short");
    assert!(events.try_recv().is_err(), "it reconnected after the close");
}

#[test]
fn only_a_renewal_or_restart_skips_the_first_backoff() {
    assert!(skips_backoff(true, false, 1) && skips_backoff(false, true, 1));
    assert!(!skips_backoff(false, false, 1));
    assert!(!skips_backoff(true, true, 2), "a failed first attempt must back off");
}

#[tokio::test]
async fn a_request_racing_close_is_reported_to_the_observer() {
    let (mut r, _frames, _updates) = runner().await;
    let seen = observe(&mut r);
    // Closed without the wake, so only the request can end the loop.
    r.control.closed.store(true, std::sync::atomic::Ordering::Release);
    let (requests, rx) = mpsc::unbounded_channel();
    let (answer, waiting) = oneshot::channel();
    requests.send(Request { name: "ping", body: ping(1), tag: None, answer }).unwrap();
    tokio::time::timeout(Duration::from_secs(5), r.run(rx)).await.expect("the runner kept going");
    assert!(matches!(waiting.await, Ok(Err(Error::Disconnected))));
    let disconnected = Err(Error::Disconnected.to_string());
    assert_eq!(seen.lock().unwrap().iter().filter(|s| s.0 == "ping" && s.2 == disconnected).count(), 1);
}

#[test]
fn restart_after_close_is_refused() {
    let (requests, _open) = mpsc::unbounded_channel();
    let session = Session::local(requests);
    assert!(session.restart().is_ok());
    session.close();
    assert!(matches!(session.restart(), Err(Error::Disconnected)));

    let (requests, gone) = mpsc::unbounded_channel();
    drop(gone);
    assert!(matches!(Session::local(requests).restart(), Err(Error::Disconnected)));
}

#[tokio::test]
async fn panicking_listener_keeps_the_connection() {
    let (mut r, mut frames, _updates) = runner().await;
    r.events.listen(Arc::new(|_| panic!("listener bug")));
    assert!(r.handle_packet(vec![0; 64]).await.is_ok());
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    assert_eq!(recv(&mut frames, &r).await.2, ping(1));
}

#[tokio::test]
async fn abandoned_requests_waiting_for_a_reconnect_never_go_out() {
    let (mut r, _frames, _updates) = runner().await;
    let seen = observe(&mut r);
    drop(request(&mut r, ping(1)));
    let _live = request(&mut r, ping(2));
    r.drop_abandoned();
    assert_eq!(r.queue.iter().map(|p| &p.body).collect::<Vec<_>>(), [&ping(2)]);
    assert!(r.service.is_empty());
    assert_eq!(*seen.lock().unwrap(), [("ping", 0, Err(Error::Timeout.to_string()))]);
}

#[tokio::test]
async fn observer_sees_how_each_request_ended() {
    let (mut r, mut frames, _updates) = runner().await;
    let seen = observe(&mut r);
    let mut answered = request(&mut r, ping(1));
    let _flooded = request(&mut r, ping(2));
    let abandoned = request(&mut r, ping(3));
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let ids: Vec<i64> = msg_container::parse(&body).unwrap().iter().map(|m| m.msg_id).collect();

    r.handle_message(1, 1, &rpc_result(ids[0], b"pong"));
    let flood = types::RpcError { error_code: 420, error_message: b"FLOOD_WAIT_5".to_vec() };
    r.handle_message(5, 1, &rpc_result(ids[1], &flood.to_bytes()));
    drop(abandoned);
    r.drop_abandoned();
    // The server answers before it sees the rpc_drop_answer: nobody is waiting any more.
    r.handle_message(9, 1, &rpc_result(ids[2], b"late"));

    assert_eq!(answered.try_recv().unwrap().unwrap(), &b"pong"[..]);
    let flood = Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_5")).to_string();
    let expected = [(ids[0], Ok(b"pong".to_vec())), (ids[1], Err(flood)), (ids[2], Err(Error::Timeout.to_string()))];
    assert_eq!(*seen.lock().unwrap(), expected.map(|(id, result)| ("ping", id, result)));
    assert!(r.pending.is_empty());
    assert_eq!(r.service.back().unwrap().0, functions::RpcDropAnswer { req_msg_id: ids[2] }.to_bytes());
}

#[tokio::test]
async fn panicking_observer_keeps_the_connection() {
    let (mut r, mut frames, _updates) = runner().await;
    Arc::make_mut(&mut r.options).observer = Some(Observer(Arc::new(|_: &Call<'_>| panic!("observer bug"))));
    // Its caller is already gone, so the runner reports it straight away.
    let (answer, _) = oneshot::channel();
    r.enqueue(Request { name: "ping", body: ping(1), tag: None, answer });
    let _rx = request(&mut r, ping(2));
    r.flush().await.unwrap();
    assert_eq!(recv(&mut frames, &r).await.2, ping(2));
}

#[tokio::test]
async fn a_failed_recording_write_is_reported() {
    let path = std::env::temp_dir().join(format!("vaegram-read-only-runner-{}.rec", std::process::id()));
    std::fs::write(&path, b"").unwrap();
    let (mut r, mut frames, _updates) = runner().await;
    Arc::make_mut(&mut r.options).record = Some(crate::storage::recording::Recorder::read_only(&path));
    let events = r.events.record();
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    r.handle_message(1, 0, &types::MsgsAck { msg_ids: Vec::new() }.to_bytes());
    std::fs::remove_file(&path).unwrap();
    let failed = events.try_iter().filter(|e| matches!(e, ClientEvent::RecordingFailed { .. })).count();
    assert_eq!(failed, 2, "one outgoing, one incoming");
}

// The next message the running runner sends, skipping its own pings.
async fn next_message(frames: &mut Frames, keys: &([u8; 256], [u8; 8], [u8; 8])) -> (i64, Vec<u8>) {
    const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
    loop {
        let packet = frames.recv().await.unwrap();
        let m = mtproto::unpack(&packet, &keys.0, &keys.1, &keys.2, false).unwrap();
        let msgs = match constructor_id(&m.body) {
            Some(msg_container::ID) => {
                msg_container::parse(&m.body).unwrap().iter().map(|c| (c.msg_id, c.body.to_vec())).collect()
            }
            _ => vec![(m.msg_id, m.body)],
        };
        if let Some(found) = msgs.into_iter().find(|(_, body)| constructor_id(body) != Some(PING_DELAY_DISCONNECT)) {
            return found;
        }
    }
}

#[tokio::test]
async fn a_timed_out_request_is_dropped_and_the_connection_carries_on() {
    let (mut r, mut frames, _updates) = runner().await;
    let seen = observe(&mut r);
    let keys = (r.auth_key, r.auth_key_id, r.session_id);
    let (requests, rx) = mpsc::unbounded_channel();
    let session = Session::local(requests);
    tokio::spawn(r.run(rx));

    let first = functions::Ping { ping_id: 1 };
    let e = session.invoke_with_timeout(&first, Duration::from_millis(50)).await.unwrap_err();
    assert!(matches!(e, Error::Timeout));
    let (msg_id, body) = next_message(&mut frames, &keys).await;
    assert_eq!(body, first.to_bytes());
    let (_, body) = next_message(&mut frames, &keys).await;
    assert_eq!(body, functions::RpcDropAnswer { req_msg_id: msg_id }.to_bytes());
    assert_eq!(*seen.lock().unwrap(), [("ping", msg_id, Err(Error::Timeout.to_string()))]);

    let waiting = tokio::spawn(async move { session.invoke(&functions::Ping { ping_id: 2 }).await });
    assert_eq!(next_message(&mut frames, &keys).await.1, ping(2));
    waiting.abort();
}

const BOOL_TRUE: u32 = 0x997275b5;

#[tokio::test]
async fn temp_key_carries_only_the_bind_until_bound() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.use_temp_key(Created { auth_key: [8; 256], salt: 5, time_offset: 0 });
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let (bind_id, seq_no, body) = recv(&mut frames, &r).await;
    assert_eq!(constructor_id(&body), Some(0xcdd42a05));
    assert_eq!(seq_no % 2, 1);
    assert!(r.unbound() && r.queue.len() == 1 && r.pending.is_empty());

    let salt = types::BadServerSalt { bad_msg_id: bind_id, bad_msg_seqno: seq_no, error_code: 48, new_server_salt: 6 };
    r.handle_message(1, 0, &salt.to_bytes());
    assert!(r.bind_due());
    r.flush().await.unwrap();
    let (rebind_id, _, body) = recv(&mut frames, &r).await;
    assert!(rebind_id > bind_id && constructor_id(&body) == Some(0xcdd42a05));

    r.handle_message(3, 1, &rpc_result(rebind_id, &BOOL_TRUE.to_le_bytes()));
    assert!(!r.unbound());
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let msgs = msg_container::parse(&body).unwrap();
    assert_eq!(msgs[1].body, ping(77));
    assert_eq!(msgs[2].body, ping(1));
}

#[tokio::test]
async fn requests_sent_under_a_lost_temp_key_are_failed_not_resent() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    r.handle_message(1, 1, &rpc_result(bind_id, &BOOL_TRUE.to_le_bytes()));
    let mut rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    assert!(r.pending.values().any(|p| p.body == ping(1)));

    r.use_temp_key(Created { auth_key: [8; 256], salt: 5, time_offset: 0 });
    r.resend_pending().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(Err(Error::Disconnected))));
    assert!(r.pending.is_empty() && r.queue.is_empty() && r.recheck.is_empty());
}

#[tokio::test]
async fn stale_perm_key_ends_the_session_like_a_404() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    let error = types::RpcError { error_code: 400, error_message: "ENCRYPTED_MESSAGE_INVALID".into() };
    r.handle_message(3, 1, &rpc_result(bind_id, &error.to_bytes()));
    let e = r.bind_failure().unwrap();
    assert!(matches!(e, Error::Transport(404)));
    assert!(r.ends_session(&e));
}

#[tokio::test]
async fn bind_retries_back_off_while_a_new_perm_key_propagates() {
    let (mut r, _frames, _updates) = runner().await;
    // Paused only now: a paused clock jumps past the connect timeout while a real socket is still connecting.
    tokio::time::pause();
    r.pfs = Some(Pfs::new([9; 256], Some(tokio::time::Instant::now()), ping(77), i64::MAX));
    let _rx = request(&mut r, ping(1));
    let error = types::RpcError { error_code: 400, error_message: "ENCRYPTED_MESSAGE_INVALID".into() };
    let binds = |r: &Runner| r.pfs.as_ref().unwrap().binds_sent.clone();
    for _ in 0..59 {
        let before = binds(&r).len();
        if r.bind_due() {
            r.flush().await.unwrap();
        }
        if let Some(&bind_id) = binds(&r).get(before) {
            r.handle_message(3, 1, &rpc_result(bind_id, &error.to_bytes()));
            r.flush().await.unwrap();
            assert_eq!(binds(&r).len(), before + 1, "a refused bind went out again at once");
        }
        tokio::time::advance(Duration::from_secs(1)).await;
    }
    assert_eq!(binds(&r).len(), 6);
    assert!(r.unbound() && r.bind_failure().is_none());
}

#[tokio::test]
async fn refused_bind_ends_the_session_instead_of_reconnecting() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], Some(tokio::time::Instant::now()), ping(77), i64::MAX));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    let error = types::RpcError { error_code: 400, error_message: "TEMP_AUTH_KEY_ALREADY_BOUND".into() };
    r.handle_message(3, 1, &rpc_result(bind_id, &error.to_bytes()));
    let e = r.bind_failure().unwrap();
    assert!(matches!(&e, Error::Rpc(rpc) if rpc.name == "TEMP_AUTH_KEY_ALREADY_BOUND"));
    assert!(r.ends_session(&e) && r.unbound() && !r.bind_due());
}

#[tokio::test]
async fn lost_bound_temp_key_reconnects_with_a_new_one() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    assert!(!r.ends_session(&Error::Disconnected));
    r.handle_message(3, 1, &rpc_result(bind_id, &BOOL_TRUE.to_le_bytes()));
    assert!(!r.temp_key_stale());
    assert!(!r.ends_session(&Error::Transport(404)) && r.temp_key_stale());
}

#[tokio::test]
async fn a_404_before_the_bind_is_answered_gets_a_new_temp_key() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    assert!(!r.ends_session(&Error::Transport(404)), "bind due");
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    assert!(r.unbound() && r.temp_key_stale());
    assert!(!r.ends_session(&Error::Transport(404)), "bind sent");
    assert!(r.temp_key_stale());
}

#[tokio::test]
async fn a_transient_error_on_the_bind_does_not_end_the_session() {
    for (code, message) in [(500, "INTERNAL"), (420, "FLOOD_WAIT_3"), (-503, "Timeout")] {
        let (mut r, mut frames, _updates) = runner().await;
        r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
        r.flush().await.unwrap();
        let (bind_id, _, _) = recv(&mut frames, &r).await;
        tokio::time::pause();
        let error = types::RpcError { error_code: code, error_message: message.into() };
        r.handle_message(3, 1, &rpc_result(bind_id, &error.to_bytes()));
        assert!(r.bind_failure().is_none(), "{message}: the session ends for good");
        assert!(!r.bind_due(), "{message}: the bind went out again at once");
        tokio::time::advance(Duration::from_secs(3)).await;
        assert!(r.bind_due(), "{message}: nothing sends the bind again");
        tokio::time::resume();
    }
}

#[tokio::test]
async fn an_undecodable_bind_answer_does_not_leave_the_bind_hanging() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    let bad_gzip = [&gzip_packed::ID.to_le_bytes()[..], &[3, 1, 2, 3]].concat();
    tokio::time::pause();
    r.handle_message(3, 1, &rpc_result(bind_id, &bad_gzip));
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(r.unbound() && r.bind_due(), "the bind stays sent forever");
}

#[tokio::test]
async fn requests_held_behind_an_unanswered_bind_get_the_link_probed() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    assert_eq!(constructor_id(&recv(&mut frames, &r).await.2), Some(test_dc::BIND));
    assert_eq!(r.queue.len(), 1);
    tokio::time::pause();
    tokio::time::advance(PROBE_DEAD_AFTER + Duration::from_secs(1)).await;
    assert!(matches!(r.tick().await, Err(Error::Disconnected)));
}

#[tokio::test]
async fn an_unanswered_bind_is_sent_again() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    tokio::time::pause();
    tokio::time::advance(r.options.request_timeout + Duration::from_secs(1)).await;
    // Pongs keep coming, so the link looks alive.
    r.heard();
    r.tick().await.unwrap();
    assert_eq!(constructor_id(&recv(&mut frames, &r).await.2), Some(test_dc::BIND));
    assert_eq!(r.pfs.as_ref().unwrap().binds_sent.len(), 2);
}

#[tokio::test]
async fn a_request_timeout_of_duration_max_does_not_break_the_bind() {
    let (mut r, mut frames, _updates) = runner().await;
    Arc::make_mut(&mut r.options).request_timeout = Duration::MAX;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
    r.flush().await.unwrap();
    assert_eq!(constructor_id(&recv(&mut frames, &r).await.2), Some(test_dc::BIND));
}

#[tokio::test]
async fn a_bad_msg_notification_for_the_bind_sends_it_again() {
    for error_code in [16, 17, 32, 33] {
        let (mut r, mut frames, _updates) = runner().await;
        r.pfs = Some(Pfs::new([9; 256], None, ping(77), i64::MAX));
        r.flush().await.unwrap();
        let (bad_msg_id, bad_msg_seqno, _) = recv(&mut frames, &r).await;
        assert!(!r.bind_due());
        let bad = types::BadMsgNotification { bad_msg_id, bad_msg_seqno, error_code };
        r.handle_message(1, 0, &bad.to_bytes());
        assert!(r.bind_due(), "error {error_code}");
        r.flush().await.unwrap();
        assert_eq!(constructor_id(&recv(&mut frames, &r).await.2), Some(test_dc::BIND), "error {error_code}");
    }
}

#[tokio::test]
async fn a_busy_session_holds_new_requests_so_its_temp_key_can_be_renewed() {
    let (mut r, mut frames, _updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), internals::now() + 7200));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    r.handle_message(1, 1, &rpc_result(bind_id, &BOOL_TRUE.to_le_bytes()));
    let _busy = request(&mut r, ping(1));
    r.flush().await.unwrap();
    let _ = recv(&mut frames, &r).await;
    let init = r.pending.iter().find(|(_, p)| p.body == ping(77)).map(|(&id, _)| id).unwrap();
    r.handle_message(3, 1, &rpc_result(init, b"config"));
    assert!(!r.temp_key_expiring());

    // An hour before the key expires.
    r.set_time_offset(7200 - 3600);
    let mut held = request(&mut r, ping(2));
    r.flush().await.unwrap();
    assert_eq!(r.queue.len(), 1, "a new request went out under the expiring key");
    assert!(!r.temp_key_expiring(), "a request still waits for its answer");
    let busy = *r.pending.keys().next().unwrap();
    r.handle_message(5, 1, &rpc_result(busy, b"pong"));
    assert!(r.temp_key_expiring());
    assert!(held.try_recv().is_err());
}

#[tokio::test]
async fn renewing_the_temp_key_is_not_reported_as_a_reconnect() {
    let dc = TestDc::start([9; 256], false).await;
    let (mut r, mut frames, mut updates) = runner().await;
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), internals::now() + 7200));
    r.flush().await.unwrap();
    let (bind_id, _, _) = recv(&mut frames, &r).await;
    r.handle_message(1, 1, &rpc_result(bind_id, &BOOL_TRUE.to_le_bytes()));
    r.set_time_offset(7200 - 3600);
    assert!(r.temp_key_expiring());

    r.dc = dc.dc(2);
    let (events, links) = (r.events.record(), r.events.record_link());
    let (session_id, salt) = (r.session_id, r.salt());
    let (_sender, requests) = mpsc::unbounded_channel();
    r.reconnect(&requests, true, false).await;
    assert!(!events.try_iter().any(|e| matches!(e, ClientEvent::Reconnected { .. })));
    assert!(!r.events.take_reconnected());
    assert!(links.try_iter().next().is_none(), "a renewal reported a link change");
    assert!(r.session_id != session_id && r.salt() != salt && r.unbound());
    assert!(matches!(updates.recv().await, Some(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
}

// Reconnects `r` as a restart does, against a listener that accepts once, and returns the link changes seen.
async fn restart_links(mut r: Runner, listener: TcpListener) -> Vec<bool> {
    let links = r.events.record_link();
    let server = tokio::spawn(async move { listener.accept().await.unwrap().0 });
    let (_sender, requests) = mpsc::unbounded_channel();
    r.reconnect(&requests, false, true).await;
    let _server = server.await.unwrap();
    links.try_iter().collect()
}

#[tokio::test]
async fn restart_reports_down_then_up_once() {
    let (r, _frames, _updates, listener) = listening(Salt::default()).await;
    assert_eq!(restart_links(r, listener).await, [false, true]);
}

#[tokio::test]
async fn transfer_connections_report_no_link_changes() {
    let (mut r, _frames, _updates, listener) = listening(Salt::default()).await;
    r.updates = None;
    assert!(restart_links(r, listener).await.is_empty());
}

#[tokio::test]
async fn closing_reports_down_but_dropping_the_handles_does_not() {
    let guard = Duration::from_secs(5);
    let (r, _frames, _updates) = runner().await;
    let (links, control) = (r.events.record_link(), r.control.clone());
    let (_requests, rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(r.run(rx));
    control.close();
    tokio::time::timeout(guard, task).await.unwrap().unwrap();
    assert_eq!(links.try_iter().collect::<Vec<_>>(), [false]);

    let (r, _frames, _updates) = runner().await;
    let links = r.events.record_link();
    let (requests, rx) = mpsc::unbounded_channel::<Request>();
    let task = tokio::spawn(r.run(rx));
    drop(requests);
    tokio::time::timeout(guard, task).await.unwrap().unwrap();
    assert!(links.try_iter().next().is_none(), "a client drop or DC switch looked like a lost connection");
}

#[tokio::test]
async fn is_connected_follows_the_link_and_ends_with_close() {
    let (mut r, _frames, _updates, listener) = listening(Salt::default()).await;
    let mut session = Session::local(mpsc::unbounded_channel().0);
    session.control = r.control.clone();
    assert!(session.is_connected(), "a fresh runner has a connection");

    // What each link change looked like to is_connected at the moment the hooks heard of it.
    let (seen, control) = (Arc::new(Mutex::new(Vec::new())), r.control.clone());
    let log = seen.clone();
    r.events.on_link(Arc::new(move |up| {
        log.lock().unwrap().push((up, control.connected.load(std::sync::atomic::Ordering::Acquire)));
    }));
    let server = tokio::spawn(async move { listener.accept().await.unwrap().0 });
    let (_sender, requests) = mpsc::unbounded_channel();
    r.reconnect(&requests, false, true).await;
    let _server = server.await.unwrap();
    assert_eq!(*seen.lock().unwrap(), [(false, false), (true, true)]);
    assert!(session.is_connected());

    let task = tokio::spawn(r.run(requests));
    session.close();
    assert!(!session.is_connected(), "close is seen before the task gets to it");
    tokio::time::timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
    assert!(!session.control.connected.load(std::sync::atomic::Ordering::Acquire));
}

#[tokio::test]
async fn a_silent_server_does_not_pin_the_pfs_reconnect() {
    let (mut r, _frames, _updates, listener) = listening(Salt::default()).await;
    // Already expired, so the reconnect runs a new key exchange on the fresh connection.
    r.pfs = Some(Pfs::new([9; 256], None, ping(77), 0));
    let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = accepted.clone();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((s, _)) = listener.accept().await {
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            held.push(s);
        }
    });
    let (tx, requests) = mpsc::unbounded_channel::<Request>();
    let closer = async {
        while accepted.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        // Connected: from here a paused clock runs straight to the next deadline.
        tokio::time::pause();
        drop(tx);
    };
    let reconnect = r.reconnect(&requests, false, false);
    let done = tokio::time::timeout(Duration::from_secs(60), async { tokio::join!(reconnect, closer) }).await;
    assert!(done.is_ok(), "the runner sits in the key exchange forever");
}

fn calls(dc: &TestDc, connection: usize) -> Vec<Vec<u32>> {
    dc.seen().into_iter().filter(|s| s.connection == connection).map(|s| s.ids).collect()
}

#[tokio::test]
async fn a_pfs_client_binds_every_connection_it_opens() {
    use test_dc::{BIND, EXPORT_AUTHORIZATION, GET_CONFIG, GET_NEAREST_DC, IMPORT_AUTHORIZATION, INIT_CONNECTION};
    use test_dc::{INVOKE_WITH_LAYER as LAYER, PING};
    let dc = TestDc::start([5; 256], false).await;
    let client = crate::Client::builder("pfs", 1, "pfs").session_string(dc.session_string([5; 256])).pfs();
    let client = client.connect().await.unwrap();
    let (bind, nearest) = (vec![BIND], vec![LAYER, INIT_CONNECTION, GET_NEAREST_DC]);
    let connected = [bind.clone(), nearest.clone(), vec![LAYER, INIT_CONNECTION, GET_CONFIG]];
    assert_eq!(calls(&dc, 1), connected);

    // A migration makes a new permanent key, and binds a temporary one to it too.
    client.switch_dc(4).await.unwrap();
    assert_eq!(calls(&dc, 2), connected);

    let lease = client.transfer_sessions(client.dc(2).unwrap(), 2).await.unwrap();
    for session in &lease.sessions {
        session.invoke(&functions::Ping { ping_id: 1 }).await.unwrap();
    }
    assert_eq!(calls(&dc, 2)[3..], [vec![EXPORT_AUTHORIZATION]]);
    let import = vec![LAYER, INIT_CONNECTION, IMPORT_AUTHORIZATION];
    assert_eq!(calls(&dc, 3), [bind.clone(), nearest.clone(), import, vec![PING]]);
    assert_eq!(calls(&dc, 4), [bind, nearest, vec![PING]]);
    assert!(dc.seen().iter().all(|s| s.temp), "a call went out under a permanent key");
}

#[tokio::test]
async fn every_connection_on_a_new_permanent_key_waits_for_it_to_propagate() {
    let dc = TestDc::start([5; 256], true).await;
    let export = types::auth::ExportedAuthorization { id: 1, bytes: vec![1] };
    let export = base::auth::ExportedAuthorization::ExportedAuthorization(Box::new(export)).to_bytes();
    let (home, _sent) = Session::scripted(vec![Ok(export)]);
    let init = super::init_connection(1, &Options::default(), functions::help::GetNearestDc {}).to_bytes();
    let home = Session { pfs: Some(Arc::from(init)), ..home };
    let options = Options::new().transport(crate::connection::Transport::Intermediate);
    let pool = TransferPool::default();
    let lease = pool.lease(dc.dc(4), 2, Auth::Home(&home, 1), &options, &Arc::default()).await.unwrap();
    for (i, session) in lease.sessions.iter().enumerate() {
        let pong = session.invoke(&functions::Ping { ping_id: 1 }).await;
        assert!(pong.is_ok(), "connection {}: {pong:?}", i + 1);
    }
}

#[tokio::test]
async fn after_log_out_a_transfer_lease_authorizes_again() {
    use test_dc::{IMPORT_AUTHORIZATION, INIT_CONNECTION, INVOKE_WITH_LAYER as LAYER};
    let dc = TestDc::start([5; 256], false).await;
    let logged_out = base::auth::LoggedOut::LoggedOut(Box::new(types::auth::LoggedOut { future_auth_token: None }));
    let export = types::auth::ExportedAuthorization { id: 1, bytes: vec![1] };
    let export = base::auth::ExportedAuthorization::ExportedAuthorization(Box::new(export));
    let (client, mut home_sent) = crate::Client::scripted(vec![Ok(logged_out.to_bytes()), Ok(export.to_bytes())]);
    let (imported, mut on_imported) = Session::scripted(Vec::new());
    client.0.transfers.insert(dc.dc(4), &imported);

    client.log_out().await.unwrap();
    let options = Options::new().transport(crate::connection::Transport::Intermediate);
    let home = client.session();
    let lease = client.0.transfers.lease(dc.dc(4), 1, Auth::Home(&home, 1), &options, &Arc::default()).await;
    drop(lease.unwrap());

    let sent: Vec<Vec<u8>> = std::iter::from_fn(|| home_sent.try_recv().ok()).collect();
    let export = functions::auth::ExportAuthorization { dc_id: 4 }.to_bytes();
    assert_eq!(sent, [functions::auth::LogOut {}.to_bytes(), export]);
    assert_eq!(calls(&dc, 1), [vec![LAYER, INIT_CONNECTION, IMPORT_AUTHORIZATION]]);
    assert!(on_imported.try_recv().is_err());
}

#[tokio::test]
async fn saved_clock_offset_is_confirmed_by_the_server() {
    let (mut r, _frames, _updates) = runner_with(Salt { value: 9, time_offset: -3600 }).await;
    r.clock_synced = false;
    let server_now = crate::session::internals::now() - 3600;
    assert!(r.fresh((server_now << 32) | 1, &ping(1)));
    assert!(r.clock_synced);
    assert!(!r.fresh(((server_now - 600) << 32) | 1, &ping(1)));
}

// A saved offset that is wrong by less than the window is never corrected; what the server's messages show is kept
// beside it.
#[tokio::test]
async fn the_server_clock_is_read_off_accepted_messages() {
    let (mut r, _frames, _updates) = runner_with(Salt { value: 9, time_offset: 100 }).await;
    let server_now = crate::session::internals::now();
    assert!(r.fresh((server_now << 32) | 1, &ping(1)));
    assert!(!r.fresh(((server_now - 600) << 32) | 1, &ping(1)), "a refused message shows nothing");
    let session = Session { learned: r.learned.clone(), ..Session::local(mpsc::unbounded_channel().0) };
    assert!(session.seen_offset().abs() <= 1, "{}", session.seen_offset());
    assert_eq!(session.salt().time_offset, 100);
}

// Reads the salt out of a client frame on the intermediate transport.
async fn first_salt(listener: &TcpListener, auth_key: &[u8; 256]) -> i64 {
    let (mut server, _) = listener.accept().await.unwrap();
    server.read_exact(&mut [0; 4]).await.unwrap();
    let mut frame = vec![0; server.read_u32_le().await.unwrap() as usize];
    server.read_exact(&mut frame).await.unwrap();
    let msg_key: [u8; 16] = frame[8..24].try_into().unwrap();
    let (key, iv) = mtproto::kdf(auth_key, &msg_key, true);
    let mut plain = frame[24..].to_vec();
    warpcrypto::ige256_decrypt(&mut plain, &key, &iv).unwrap();
    i64::from_le_bytes(plain[..8].try_into().unwrap())
}

async fn transfer_dc() -> (TcpListener, Dc, Options) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let dc = Dc { id: 4, addr: listener.local_addr().unwrap(), test: false, media: false };
    (listener, dc, Options::new().transport(crate::connection::Transport::Intermediate))
}

#[tokio::test]
async fn transfer_connections_start_with_the_salt_they_are_given() {
    let (listener, dc, options) = transfer_dc().await;
    let salt = Salt { value: 0x5a17, time_offset: 0 };
    let key = Known { key: [7; 256], salt, made: None };
    let session = Session::connect_transfer(dc, Some(key), None, &options, Arc::default(), None, 1);
    let session = session.await.unwrap();
    tokio::spawn(async move { session.invoke(&functions::Ping { ping_id: 1 }).await });
    assert_eq!(first_salt(&listener, &[7; 256]).await, 0x5a17);
}

#[tokio::test]
async fn leased_connections_start_with_the_salt_the_slot_learned() {
    let (listener, dc, options) = transfer_dc().await;
    let (requests, _open) = mpsc::unbounded_channel();
    let learned = Arc::new(Learned { salt: 77.into(), time_offset: 0.into(), seen_offset: 0.into() });
    let live = Session { learned, ..Session::local(requests) };
    let pool = TransferPool::default();
    pool.insert(dc, &live);
    // Learned after the slot stored 77: the live connection's current salt wins.
    live.learned.salt.store(78, std::sync::atomic::Ordering::Relaxed);
    let lease = pool.lease(dc, 2, Auth::Home(&Session::offline(), 1), &options, &Arc::default()).await.unwrap();
    let opened = lease.sessions[1].clone();
    tokio::spawn(async move { opened.invoke(&functions::Ping { ping_id: 1 }).await });
    assert_eq!(first_salt(&listener, live.auth_key()).await, 78);
}

/// The counts of the `UpdatesDropped` reported when `update` comes to a full queue.
async fn dropped_with(update: base::Update) -> (u32, u32) {
    let (mut r, _frames, mut updates) = runner().await;
    let events = r.events.record();
    let empty = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
    let capacity = Options::default().updates_capacity as i64;
    for id in 0..capacity {
        r.handle_message(id, 0, &base::Updates::Updates(Box::new(empty.clone())).to_bytes());
    }
    let update = types::UpdateShort { update, date: 0 };
    r.handle_message(capacity, 0, &base::Updates::UpdateShort(Box::new(update)).to_bytes());
    assert!(matches!(updates.recv().await.unwrap().updates, base::Updates::UpdatesTooLong));
    match events.try_recv() {
        Ok(ClientEvent::UpdatesDropped { count, unrecoverable }) => (count, unrecoverable),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn dropped_updates_without_pts_are_counted_as_lost() {
    let query = types::UpdateBotCallbackQuery {
        query_id: 1,
        user_id: 7,
        peer: crate::utils::get_peer(7),
        msg_id: 1,
        chat_instance: 0,
        data: None,
        game_short_name: None,
    };
    assert_eq!(dropped_with(base::Update::UpdateBotCallbackQuery(Box::new(query))).await, (1, 1));
}

// getDifference brings back what moved pts, so a dropped deletion numbered 5 is not lost; one numbered 0 moved
// nothing and is.
#[tokio::test]
async fn dropped_updates_with_a_pts_of_0_are_counted_as_lost() {
    let deleted = |pts| {
        let update = types::UpdateDeleteMessages { messages: vec![1], pts, pts_count: 1 };
        base::Update::UpdateDeleteMessages(Box::new(update))
    };
    assert_eq!(dropped_with(deleted(5)).await, (1, 0));
    assert_eq!(dropped_with(deleted(0)).await, (1, 1));
}

type Contexts = Arc<Mutex<Vec<(i32, u32, u16, Option<u64>)>>>;

fn observe_context(options: &mut Options) -> Contexts {
    let seen = Contexts::default();
    let log = seen.clone();
    options.observer =
        Some(Observer(Arc::new(move |c: &Call<'_>| log.lock().unwrap().push((c.dc, c.connection, c.resent, c.tag)))));
    seen
}

#[tokio::test]
async fn observer_sees_the_connection_and_resends() {
    let (mut r, mut frames, _updates) = runner().await;
    let seen = observe_context(Arc::make_mut(&mut r.options));
    drop(request(&mut r, ping(1)));
    r.drop_abandoned();

    let (answer, mut answered) = oneshot::channel();
    r.enqueue(Request { name: "ping", body: ping(2), tag: Some(6), answer });
    r.flush().await.unwrap();
    let (bad_msg_id, bad_msg_seqno, _) = recv(&mut frames, &r).await;
    let salt = types::BadServerSalt { bad_msg_id, bad_msg_seqno, error_code: 48, new_server_salt: 1 };
    r.handle_message(1, 0, &salt.to_bytes());
    r.flush().await.unwrap();
    let (msg_id, _, body) = recv(&mut frames, &r).await;
    assert_eq!(body, ping(2));
    r.handle_message(5, 1, &rpc_result(msg_id, b"pong"));
    assert!(answered.try_recv().unwrap().is_ok());
    // The re-send keeps the tag of the task that sent it first.
    assert_eq!(*seen.lock().unwrap(), [(2, 0, 0, None), (2, 0, 1, Some(6))]);
}

#[tokio::test]
async fn leased_connections_are_numbered_apart() {
    let (_listener, dc, mut options) = transfer_dc().await;
    let seen = observe_context(&mut options);
    let pool = TransferPool::default();
    // Its request channel is closed, so the slot keeps only its key.
    pool.insert(dc, &Session::offline());
    let lease = pool.lease(dc, 2, Auth::Home(&Session::offline(), 1), &options, &Arc::default()).await.unwrap();
    for session in &lease.sessions {
        let e = session.invoke_with_timeout(&functions::Ping { ping_id: 1 }, Duration::from_millis(1)).await;
        assert!(matches!(e, Err(Error::Timeout)));
    }
    // Each runner reports its abandoned ping on its next tick.
    let reported = async {
        while seen.lock().unwrap().len() < 2 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), reported).await.expect("a runner never reported its ping");
    let mut seen = seen.lock().unwrap().clone();
    seen.sort_unstable();
    assert_eq!(seen, [(4, 1, 0, None), (4, 2, 0, None)]);
}

/// A stream that counts its reads, for telling a connection that waits from one that spins.
struct CountedReads(tokio::io::DuplexStream, Arc<std::sync::atomic::AtomicUsize>);

impl tokio::io::AsyncRead for CountedReads {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.1.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::pin::Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for CountedReads {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_main_connection_nobody_takes_updates_from_still_reconnects() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (ours, theirs) = tokio::io::duplex(1 << 16);
    let (reads, dials) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let first = Mutex::new(Some(CountedReads(ours, reads.clone())));
    let dialed = dials.clone();
    // The first dial gets the pipe; later ones are refused, as by a network that's still down.
    let factory = move |_: std::net::SocketAddr| {
        dialed.fetch_add(1, Ordering::Relaxed);
        let stream = first.lock().unwrap().take();
        async move { stream.ok_or_else(|| std::io::Error::other("still down")) }
    };
    let options = Options::new().transport(crate::connection::Transport::Intermediate);
    let options = Options { connection_factory: Some(Arc::new(factory)), ..options };
    let dc = Dc { id: 2, addr: "127.0.0.1:1".parse().unwrap(), test: false, media: false };
    let conn = Connection::connect(&dc, &options).await.unwrap();
    let (tx, rx) = mpsc::channel(8);
    let r = Runner::new(dc, Arc::new(options), conn, ([7; 256], Salt::default(), true), Arc::default(), Some(tx));
    // Nobody takes this connection's updates any more, but its requests still have a caller.
    drop(rx);
    let (_requests, requests) = mpsc::unbounded_channel();
    let task = tokio::spawn(r.run(requests));
    drop(theirs);
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let (reads, dials) = (reads.load(Ordering::Relaxed), dials.load(Ordering::Relaxed));
    task.abort();
    assert!(dials >= 2, "it never dialed again");
    assert!(reads < 1000, "it spun on the dead link: {reads} reads");
}
