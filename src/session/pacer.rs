//! How many connections a file transfer spreads its parts over, and how fast it may send them.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::{Semaphore, SemaphorePermit};
use tokio::time::Instant;

use crate::errors::{Error, Result};

/// The most connections one transfer opens: what Premium accounts already got, and the most measured.
pub(crate) const MAX_CONNECTIONS: usize = 8;

/// A transfer of this many bytes or more by a bot gets the bot shape, the pace and one getFile per connection.
pub(crate) const LARGE: u64 = 8 << 20;

// Bots, measured 2026-11-14 with one bot from one server 61 ms from DC5, 7 runs of 500 MiB and one of 2,000 MiB:
// 8 connections paced to 20 MiB/s down, one getFile out on each, and 19.5 MiB/s up, two parts out on each, gave a
// median 19.88 MB/s down and 20.14 up with 0-3 and 0-7 FLOOD_WAITs a run, against 17.65 and 17.79 (28-45 upload
// FLOOD_WAITs) on 4 unpaced; the 2,000 MiB run went at 20.25 and 20.37. wzgram paces every bot at 40 parts a second,
// the same 20 MiB/s. Nothing is measured on other DCs, nor for several processes on one bot token: the pace is per
// client and per DC. Those are sustained transfers, and nothing shows a short burst floods, so a bot's transfer
// under LARGE goes as a user's, unpaced, and doesn't wait for 8 handshakes before its first byte. So does a download
// of unknown size, which is anything fetched by file_id, a large file too, and chat photos: as before, its chunks
// go one at a time on one connection, and a pace or a gate would only queue many of them behind each other. User
// accounts are not measured at all, so they keep the shape they had, unpaced.
const BOT: Shapes = Shapes { download: Shape::new(8, 1), upload: Shape::new(8, 2) };
const BOT_DOWNLOAD_RATE: u64 = 20 << 20;
const BOT_UPLOAD_RATE: u64 = 39 << 19;
// 2 MiB of 512 KiB chunks on 4 connections down, 4 MiB of 512 KiB parts on 4 up.
const USER: Shapes = Shapes { download: Shape::new(4, 1), upload: Shape::new(4, 2) };
// The server throttles Premium transfers less: 16 MiB down and 4 MiB up, on 8 connections each.
const PREMIUM: Shapes = Shapes { download: Shape::new(8, 4), upload: Shape::new(8, 1) };

/// The way a file part goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Direction {
    Download,
    Upload,
}

/// How a transfer spreads its parts: over `connections` connections, `per_connection` parts in flight on each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shape {
    pub(crate) connections: usize,
    pub(crate) per_connection: usize,
}

impl Shape {
    const fn new(connections: usize, per_connection: usize) -> Shape {
        Shape { connections, per_connection }
    }

    /// The bytes the transfer keeps in flight, in parts of `part_size`.
    pub(crate) fn in_flight(self, part_size: u64) -> u64 {
        (self.connections * self.per_connection) as u64 * part_size
    }
}

struct Shapes {
    download: Shape,
    upload: Shape,
}

/// How one transfer goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) shape: Shape,
    /// Bytes a second, shared with every transfer to the same DC in the same direction; `None` for unpaced.
    pub(crate) rate: Option<u64>,
    /// Whether each upload.getFile holds its connection's [`Gate`] while it's out.
    pub(crate) gated: bool,
}

/// One direction's builder overrides; `None` keeps the account's default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Lane {
    pub(crate) connections: Option<usize>,
    pub(crate) max_bytes_per_sec: Option<Option<u64>>,
}

/// The transfer options set on the builder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TransferSettings {
    pub(crate) download: Lane,
    pub(crate) upload: Lane,
}

impl TransferSettings {
    pub(crate) fn validate(&self) -> Result<()> {
        for lane in [self.download, self.upload] {
            if lane.connections.is_some_and(|n| !(1..=MAX_CONNECTIONS).contains(&n)) {
                return Err(Error::InvalidArgument("transfer connections must be 1 to 8"));
            }
            if lane.max_bytes_per_sec == Some(Some(0)) {
                return Err(Error::InvalidArgument("max bytes per second must be above 0"));
            }
        }
        Ok(())
    }

    fn lane(&self, direction: Direction) -> Lane {
        match direction {
            Direction::Download => self.download,
            Direction::Upload => self.upload,
        }
    }

    /// How a transfer of `bytes` in `direction` goes for a bot or a user account with or without Premium; `None`
    /// bytes, as for a download of unknown size, count as small. Only large transfers are paced.
    pub(crate) fn plan(&self, direction: Direction, bot: bool, premium: bool, bytes: Option<u64>) -> Plan {
        let large = bytes.is_some_and(|bytes| bytes >= LARGE);
        let bot = bot && large;
        let rate = if large { self.rate(direction, bot) } else { None };
        Plan { shape: self.shape(direction, bot, premium), rate, gated: bot && direction == Direction::Download }
    }

    /// The shape a download from a CDN gets, whatever the account: nothing is measured there. Without the builder's
    /// download connections, the 4 connections it had before, 8 with Premium.
    pub(crate) fn cdn_shape(&self, premium: bool) -> Shape {
        self.shape(Direction::Download, false, premium)
    }

    fn shape(&self, direction: Direction, bot: bool, premium: bool) -> Shape {
        let shapes = match (bot, premium) {
            (true, _) => &BOT,
            (false, true) => &PREMIUM,
            (false, false) => &USER,
        };
        let shape = match direction {
            Direction::Download => shapes.download,
            Direction::Upload => shapes.upload,
        };
        Shape { connections: self.lane(direction).connections.unwrap_or(shape.connections), ..shape }
    }

    fn rate(&self, direction: Direction, bot: bool) -> Option<u64> {
        let default = match (bot, direction) {
            (true, Direction::Download) => Some(BOT_DOWNLOAD_RATE),
            (true, Direction::Upload) => Some(BOT_UPLOAD_RATE),
            (false, _) => None,
        };
        self.lane(direction).max_bytes_per_sec.unwrap_or(default)
    }
}

/// How one part waits its turn before each send: on `pacer` for its `bytes`, and on its connection's [`Gate`]
/// when `gated`.
#[derive(Clone, Copy, Default)]
pub(crate) struct Pace<'a> {
    pub(crate) pacer: Option<&'a Pacer>,
    pub(crate) bytes: u64,
    pub(crate) gated: bool,
}

/// Lets one gated upload.getFile out on a connection at a time, whichever download it's for: the bot shape has
/// one in flight on each, and more drew 27-78 getFile FLOOD_WAITs per 500 MiB at any pace.
pub(crate) struct Gate(Semaphore);

impl Default for Gate {
    fn default() -> Gate {
        Gate(Semaphore::new(1))
    }
}

impl Gate {
    pub(crate) async fn enter(&self) -> Option<SemaphorePermit<'_>> {
        self.0.acquire().await.ok()
    }
}

/// What a pacer lets through at once: one 512 KiB part, the download chunk and the upload part both.
const BURST: u64 = 512 << 10;

/// Spaces out the sends of the parts one client sends in one direction to one DC, so they go at `rate` bytes a
/// second: a token bucket that holds one part and starts full. An idle pacer lets a part through at once, and
/// small ones several, while parts that keep coming leave one part's time apart, not in bursts. Every send takes
/// its own bytes, a resend too.
pub(crate) struct Pacer {
    rate: u64,
    bucket: Mutex<Bucket>,
}

struct Bucket {
    // When the bucket is full again, given what the bookings so far took.
    full: Instant,
    // The bookings since the last one sent, oldest first, each with where it started taking.
    booked: VecDeque<Booking>,
    next_id: u64,
}

struct Booking {
    id: u64,
    from: Instant,
    state: State,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting,
    Sent,
    Dropped,
}

impl Bucket {
    // Nothing up to a booking that went can be handed back any more.
    fn prune(&mut self) {
        if let Some(sent) = self.booked.iter().rposition(|b| b.state == State::Sent) {
            self.booked.drain(..=sent);
        }
    }
}

/// A booking still waiting for its turn; dropped before it came, it hands its bytes back.
struct Booked<'a> {
    pacer: &'a Pacer,
    id: u64,
    sent: bool,
}

impl Drop for Booked<'_> {
    fn drop(&mut self) {
        self.pacer.settle(self.id, if self.sent { State::Sent } else { State::Dropped });
    }
}

impl Pacer {
    pub(crate) fn new(rate: u64) -> Pacer {
        let bucket = Bucket { full: Instant::now(), booked: VecDeque::new(), next_id: 0 };
        Pacer { rate, bucket: Mutex::new(bucket) }
    }

    fn time(&self, bytes: u64) -> Duration {
        Duration::from_nanos(bytes.saturating_mul(1_000_000_000) / self.rate)
    }

    /// Returns once a part of `bytes` may go.
    ///
    /// Dropped while it waits, it gives its bytes back once every booking made after it was dropped too: the next
    /// booking then starts where the earliest of them did, which is never before the end of a booking still waiting
    /// or already sent, so no part ever goes sooner than the pace lets it, and none already waiting goes later. A
    /// dropped booking with a live one after it stays charged.
    pub(crate) async fn pace(&self, bytes: u64) {
        let now = Instant::now();
        let (id, wait) = {
            let mut bucket = self.bucket.lock().unwrap();
            let from = bucket.full.max(now);
            bucket.full = from + self.time(bytes);
            let wait = bucket.full.saturating_duration_since(now).saturating_sub(self.time(BURST));
            let id = bucket.next_id;
            bucket.next_id += 1;
            let state = if wait.is_zero() { State::Sent } else { State::Waiting };
            bucket.booked.push_back(Booking { id, from, state });
            bucket.prune();
            (id, wait)
        };
        if wait.is_zero() {
            return;
        }
        let mut booked = Booked { pacer: self, id, sent: false };
        tokio::time::sleep_until(now + wait).await;
        booked.sent = true;
    }

    fn settle(&self, id: u64, state: State) {
        let mut bucket = self.bucket.lock().unwrap();
        if let Some(booking) = bucket.booked.iter_mut().find(|b| b.id == id) {
            booking.state = state;
        }
        while let Some(from) = bucket.booked.back().filter(|b| b.state == State::Dropped).map(|b| b.from) {
            bucket.full = from;
            bucket.booked.pop_back();
        }
        bucket.prune();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::{Direction, LARGE, Lane, MAX_CONNECTIONS, Pacer, Plan, Shape, TransferSettings};
    use crate::errors::Error;

    const PART: u64 = 512 << 10;

    fn plan(shape: Shape, rate: Option<u64>, gated: bool) -> Plan {
        Plan { shape, rate, gated }
    }

    #[test]
    fn a_user_account_keeps_its_shape_unpaced() {
        let settings = TransferSettings::default();
        let user = |direction, premium| settings.plan(direction, false, premium, Some(LARGE));
        assert_eq!(user(Direction::Download, false), plan(Shape::new(4, 1), None, false));
        assert_eq!(user(Direction::Upload, false), plan(Shape::new(4, 2), None, false));
        assert_eq!(user(Direction::Download, true), plan(Shape::new(8, 4), None, false));
        assert_eq!(user(Direction::Upload, true), plan(Shape::new(8, 1), None, false));
        // The bytes in flight before shapes: 2 MiB down, 16 MiB with Premium, 4 MiB up either way.
        let in_flight = |direction, premium| user(direction, premium).shape.in_flight(PART);
        assert_eq!(in_flight(Direction::Download, false), 2 << 20);
        assert_eq!(in_flight(Direction::Download, true), 16 << 20);
        assert_eq!((in_flight(Direction::Upload, false), in_flight(Direction::Upload, true)), (4 << 20, 4 << 20));
        assert_eq!(settings.cdn_shape(false), Shape::new(4, 1));
        assert_eq!(settings.cdn_shape(true), Shape::new(8, 4));
    }

    #[test]
    fn a_bot_gets_8_connections_paced_from_8_mib() {
        let settings = TransferSettings::default();
        let bot = |direction, bytes| settings.plan(direction, true, false, bytes);
        let rate = 19 * (1 << 20) + (1 << 19);
        assert_eq!(bot(Direction::Download, Some(LARGE)), plan(Shape::new(8, 1), Some(20 << 20), true));
        assert_eq!(bot(Direction::Upload, Some(LARGE)), plan(Shape::new(8, 2), Some(rate), false));
        // Under 8 MiB, or of unknown size, a bot's transfer goes as a user's.
        for direction in [Direction::Download, Direction::Upload] {
            for small in [Some(LARGE - 1), None] {
                assert_eq!(bot(direction, small), settings.plan(direction, false, false, small));
            }
        }
        assert_eq!(settings.cdn_shape(false), Shape::new(4, 1));
    }

    #[test]
    fn overrides_replace_the_defaults_and_bad_ones_are_refused() {
        let lane = |connections, rate| Lane { connections, max_bytes_per_sec: rate };
        let settings = TransferSettings { download: lane(Some(2), Some(None)), upload: lane(None, Some(Some(1000))) };
        assert!(settings.validate().is_ok());
        let large = |direction, bot, premium| settings.plan(direction, bot, premium, Some(LARGE));
        assert_eq!(large(Direction::Download, true, false), plan(Shape::new(2, 1), None, true));
        assert_eq!(large(Direction::Download, false, true), plan(Shape::new(2, 4), None, false));
        // The builder's download connections reach a CDN's too.
        assert_eq!(settings.cdn_shape(true), Shape::new(2, 4));
        assert_eq!(large(Direction::Upload, true, false), plan(Shape::new(8, 2), Some(1000), false));
        assert_eq!(large(Direction::Upload, false, false), plan(Shape::new(4, 2), Some(1000), false));
        // A small transfer isn't paced whatever the rate.
        assert_eq!(settings.plan(Direction::Upload, true, false, Some(1)).rate, None);

        for bad in [lane(Some(0), None), lane(Some(MAX_CONNECTIONS + 1), None), lane(None, Some(Some(0)))] {
            let download = TransferSettings { download: bad, ..settings };
            for settings in [download, TransferSettings { upload: bad, ..settings }] {
                assert!(matches!(settings.validate(), Err(Error::InvalidArgument(_))), "{settings:?}");
            }
        }
        assert!(TransferSettings { download: lane(Some(MAX_CONNECTIONS), None), ..settings }.validate().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn parts_go_one_parts_time_apart_and_an_idle_pacer_lets_one_through() {
        // A part a second.
        let pacer = Pacer::new(PART);
        let start = Instant::now();
        for k in 0..3 {
            pacer.pace(PART).await;
            assert_eq!(start.elapsed(), Duration::from_secs(k));
        }
        // Idle for longer than a part takes: one part goes at once, not ten.
        tokio::time::sleep(Duration::from_secs(10)).await;
        let idle = Instant::now();
        pacer.pace(PART).await;
        assert_eq!(idle.elapsed(), Duration::ZERO);
        // A quarter part waits a quarter of a part's time, and the part after it a whole one more.
        pacer.pace(PART / 4).await;
        assert_eq!(idle.elapsed(), Duration::from_millis(250));
        pacer.pace(PART).await;
        assert_eq!(idle.elapsed(), Duration::from_millis(1250));
    }

    #[tokio::test(start_paused = true)]
    async fn small_parts_fill_one_part_before_waiting() {
        let pacer = Pacer::new(PART);
        let start = Instant::now();
        for _ in 0..4 {
            pacer.pace(PART / 4).await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO);
        pacer.pace(PART / 4).await;
        assert_eq!(start.elapsed(), Duration::from_millis(250));
    }

    #[tokio::test(start_paused = true)]
    async fn parts_dropped_while_waiting_give_their_time_back() {
        let pacer = Pacer::new(PART);
        let start = Instant::now();
        // The first goes at once; the other three book 1, 2 and 3 s and are dropped, oldest first.
        let mut started: Vec<_> = (0..4).map(|_| Box::pin(pacer.pace(PART))).collect();
        for pace in &mut started {
            let _ = tokio::time::timeout(Duration::ZERO, pace.as_mut()).await;
        }
        drop(started);
        pacer.pace(PART).await;
        assert_eq!(start.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn a_part_dropped_before_a_waiting_one_stays_charged() {
        let pacer = Pacer::new(PART);
        let start = Instant::now();
        pacer.pace(PART).await;
        let mut dropped = Box::pin(pacer.pace(PART));
        let _ = tokio::time::timeout(Duration::ZERO, dropped.as_mut()).await;
        let mut waiting = Box::pin(pacer.pace(PART));
        let _ = tokio::time::timeout(Duration::ZERO, waiting.as_mut()).await;
        drop(dropped);
        // The one booked after it keeps its turn at 2 s, and the next goes after that, not in the gap.
        waiting.await;
        assert_eq!(start.elapsed(), Duration::from_secs(2));
        pacer.pace(PART).await;
        assert_eq!(start.elapsed(), Duration::from_secs(3));
    }
}
