use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use super::{
    Answer, Connection, MAX_CONTAINER_MSGS, Options, Request, Runner, Serializable, UPDATES_BUFFER, UpdatesRx, base,
    functions, mpsc, msg_container, mtproto, oneshot, outbox, types,
};
use crate::session::internals::data_center::Dc;

type Frames = mpsc::UnboundedReceiver<Vec<u8>>;

async fn runner() -> (Runner, Frames, UpdatesRx) {
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

    let (runner, updates) = Runner::new(dc, options, conn, [7; 256], 0, 0);
    (runner, frames, updates)
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
    let empty = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
    let body = base::Updates::Updates(Box::new(empty)).to_bytes();
    for id in 0..=UPDATES_BUFFER as i64 {
        r.handle_message(id, 0, &body);
    }
    let first = updates.recv().await.unwrap().updates;
    let second = updates.recv().await.unwrap().updates;
    assert!(matches!((first, second), (base::Updates::UpdatesTooLong, base::Updates::Updates(_))));
}
