//! A stream for tests that sends like a slow link: writes go into a buffer, the socket's send buffer, which drains
//! to the far end at a fixed rate of tokio's clock, so a paused clock runs it in no time. Reads come straight from the
//! far end.

use std::collections::VecDeque;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, DuplexStream, ReadBuf, ReadHalf};

pub(crate) struct SlowLink {
    read: ReadHalf<DuplexStream>,
    queue: Arc<Mutex<Queue>>,
    room: usize,
    wake: usize,
    // Flushes that still complete; `None` for all of them.
    flushes: Option<usize>,
}

#[derive(Default)]
struct Queue {
    bytes: VecDeque<u8>,
    // A writer found the buffer full and waits for `wake` bytes of room.
    blocked: bool,
    writer: Option<Waker>,
}

impl SlowLink {
    /// A link whose writes go into a buffer of `room` bytes that drains to the far end at `rate` bytes a second (not
    /// at all at 0). A writer that found the buffer full is woken once `wake` bytes are free, as Linux wakes one once
    /// about a third of its send buffer is. Returns the far end too, which gets what drained and writes back at once;
    /// it has to stay alive for anything to drain.
    pub(crate) fn new(room: usize, wake: usize, rate: usize) -> (SlowLink, DuplexStream) {
        let (near, far) = tokio::io::duplex(64 << 20);
        let (read, mut write) = tokio::io::split(near);
        let queue = Arc::new(Mutex::new(Queue::default()));
        if rate > 0 {
            let queue = Arc::clone(&queue);
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let drained: Vec<u8> = {
                        let mut queue = queue.lock().unwrap();
                        let n = queue.bytes.len().min(rate);
                        let drained = queue.bytes.drain(..n).collect();
                        if room - queue.bytes.len() >= wake {
                            queue.writer.take().into_iter().for_each(Waker::wake);
                        }
                        drained
                    };
                    if write.write_all(&drained).await.is_err() {
                        return;
                    }
                }
            });
        }
        (SlowLink { read, queue, room, wake, flushes: None }, far)
    }

    /// Lets only the next `n` flushes complete; later ones never do, as on a stream that can't get its buffer out.
    pub(crate) fn flushes(mut self, n: usize) -> SlowLink {
        self.flushes = Some(n);
        self
    }
}

impl AsyncRead for SlowLink {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().read).poll_read(cx, buf)
    }
}

impl AsyncWrite for SlowLink {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let mut queue = this.queue.lock().unwrap();
        let free = this.room - queue.bytes.len();
        if free == 0 || (queue.blocked && free < this.wake) {
            queue.blocked = true;
            queue.writer = Some(cx.waker().clone());
            return Poll::Pending;
        }
        queue.blocked = false;
        let n = free.min(buf.len());
        queue.bytes.extend(&buf[..n]);
        Poll::Ready(Ok(n))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        match &mut this.flushes {
            Some(0) => Poll::Pending,
            Some(n) => {
                *n -= 1;
                Poll::Ready(Ok(()))
            }
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
