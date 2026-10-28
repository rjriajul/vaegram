use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use super::{
    Affected, Answer, Arc, ClientEvent, Connection, Identifiable, Incoming, MAX_CONTAINER_MSGS, Options, Request,
    Runner, Serializable, UPDATES_BUFFER, UpdatesRx, base, functions, gzip_packed, mpsc, msg_container, mtproto,
    oneshot, outbox, types,
};
use crate::session::internals::data_center::Dc;

type Frames = mpsc::UnboundedReceiver<Vec<u8>>;

async fn runner() -> (Runner, Frames, UpdatesRx) {
    let (runner, frames, updates, _) = listening().await;
    (runner, frames, updates)
}

async fn listening() -> (Runner, Frames, UpdatesRx, TcpListener) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let options = Options { transport: crate::connection::Transport::Intermediate, ..Options::default() };
    let dc = Dc { id: 2, addr, test: false };
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

    let (events, (tx, rx)) = (Arc::default(), mpsc::channel(UPDATES_BUFFER));
    let runner = Runner::new(dc, options, conn, ([7; 256], 0, Some(0)), Arc::clone(&events), Some(tx));
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
    r.enqueue(Request { body, answer });
    rx
}

fn ping(ping_id: i64) -> Vec<u8> {
    functions::Ping { ping_id }.to_bytes()
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
async fn forgets_abandoned_requests() {
    let (mut r, mut frames, _updates) = runner().await;
    drop(request(&mut r, ping(1)));
    drop(request(&mut r, ping(2)));
    let mut live = request(&mut r, ping(3));
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let ids: Vec<i64> = msg_container::parse(&body).unwrap().iter().map(|m| m.msg_id).collect();

    let (answer, _) = oneshot::channel();
    r.enqueue(Request { body: ping(4), answer });
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
    assert_eq!(msg_container::parse(&body).unwrap().len(), 4);
    assert_eq!(r.pending.values().map(|p| &p.body).collect::<Vec<_>>(), [&ping(3)]);
}

#[tokio::test]
async fn reconnect_keeps_acked_requests_waiting() {
    let (mut r, mut frames, _updates) = runner().await;
    let _rx: Vec<_> = (0..2).map(|i| request(&mut r, ping(i))).collect();
    r.flush().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let ids: Vec<i64> = msg_container::parse(&body).unwrap().iter().map(|m| m.msg_id).collect();

    r.handle_message(1, 0, &types::MsgsAck { msg_ids: vec![ids[0]] }.to_bytes());
    r.resend_pending().await.unwrap();
    let (_, _, body) = recv(&mut frames, &r).await;
    let msgs = msg_container::parse(&body).unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[1].body, ping(1));
    assert!(r.pending.contains_key(&ids[0]) && r.pending.contains_key(&msgs[1].msg_id));
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
    assert_eq!((r.salt, r.queue.len(), r.pending.len()), (1, 2, 0));
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

#[tokio::test]
async fn dropped_updates_ask_for_a_difference() {
    let (mut r, _frames, mut updates) = runner().await;
    let events = r.events.record();
    let empty = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
    let body = base::Updates::Updates(Box::new(empty)).to_bytes();
    for id in 0..=UPDATES_BUFFER as i64 {
        r.handle_message(id, 0, &body);
    }
    let first = updates.recv().await.unwrap().updates;
    let second = updates.recv().await.unwrap().updates;
    assert!(matches!((first, second), (base::Updates::UpdatesTooLong, base::Updates::Updates(_))));
    assert!(matches!(events.try_recv(), Ok(ClientEvent::UpdatesDropped { count: 1 })));
}

#[tokio::test]
async fn affected_pts_come_back_as_quiet_updates() {
    let mut updates = UpdatesRx::offline();
    updates.events.affected(Affected { channel_id: Some(7), pts: 5, pts_count: 1 });
    let incoming = updates.recv().await.unwrap();
    let base::Updates::UpdateShort(short) = incoming.updates else { panic!("not a stand-in update") };
    let channel = matches!(&short.update, base::Update::UpdateDeleteChannelMessages(u) if u.channel_id == 7);
    assert_eq!((incoming.from_request, channel, short.update.pts()), (true, true, Some((5, 1))));
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
    assert!(r.handle_packet(&[0; 64]).await.is_ok());
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
    let (mut r, _frames, mut updates, listener) = listening().await;
    let events = r.events.record();
    let server = tokio::spawn(async move { listener.accept().await.unwrap().0 });
    let (_sender, requests) = mpsc::unbounded_channel();
    r.reconnect(&requests).await;
    let _server = server.await.unwrap();
    assert!(matches!(events.try_recv(), Ok(ClientEvent::Reconnected { dc: 2, attempt: 1 })));
    assert!(matches!(updates.recv().await, Some(Incoming { updates: base::Updates::UpdatesTooLong, .. })));
}

#[tokio::test]
async fn panicking_listener_keeps_the_connection() {
    let (mut r, mut frames, _updates) = runner().await;
    r.events.listen(Arc::new(|_| panic!("listener bug")));
    assert!(r.handle_packet(&[0; 64]).await.is_ok());
    let _rx = request(&mut r, ping(1));
    r.flush().await.unwrap();
    assert_eq!(recv(&mut frames, &r).await.2, ping(1));
}
