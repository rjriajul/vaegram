//! How many connections a file transfer spreads its parts over, and how fast it may send them.

use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

use crate::errors::{Error, Result};

/// The most connections one transfer opens: what Premium accounts already got, and the most measured.
pub(crate) const MAX_CONNECTIONS: usize = 8;

// Bots, measured 2026-11-14 from 61 ms to DC5, 500 MiB a run: 8 connections paced to 20 MiB/s down and 19.5 MiB/s up
// gave a median 19.88 MB/s down and 20.14 up, with 0-3 and 0-7 FLOOD_WAITs a run, against 17.65 and 17.79 (28-45
// upload FLOOD_WAITs) on 4 unpaced. Nothing is measured for user accounts, so they keep the shape they had, unpaced.
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
                return Err(Error::InvalidArgument("max_bytes_per_sec must be above 0"));
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

    /// The shape of a transfer in `direction` by a bot or a user account with or without Premium.
    pub(crate) fn shape(&self, direction: Direction, bot: bool, premium: bool) -> Shape {
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

    /// The bytes a second parts in `direction` may go at, `None` for unpaced.
    pub(crate) fn rate(&self, direction: Direction, bot: bool) -> Option<u64> {
        let default = match (bot, direction) {
            (true, Direction::Download) => Some(BOT_DOWNLOAD_RATE),
            (true, Direction::Upload) => Some(BOT_UPLOAD_RATE),
            (false, _) => None,
        };
        self.lane(direction).max_bytes_per_sec.unwrap_or(default)
    }
}

/// What a pacer lets through at once: one 512 KiB part, the download chunk and the upload part both.
const BURST: u64 = 512 << 10;

/// Spaces out the parts one client sends in one direction to one DC, so they go at `rate` bytes a second: a token
/// bucket that holds one part and starts full. An idle pacer lets a part through at once, and small files several,
/// while parts that keep coming go evenly, one part's time apart, not in bursts. Every send takes its own bytes, a
/// resend too.
pub(crate) struct Pacer {
    rate: u64,
    // When the bucket is full again, given what was taken from it so far.
    full: Mutex<Instant>,
}

impl Pacer {
    pub(crate) fn new(rate: u64) -> Pacer {
        Pacer { rate, full: Mutex::new(Instant::now()) }
    }

    pub(crate) fn rate(&self) -> u64 {
        self.rate
    }

    fn time(&self, bytes: u64) -> Duration {
        Duration::from_nanos(bytes.saturating_mul(1_000_000_000) / self.rate)
    }

    /// Returns once a part of `bytes` may go. A part dropped while waiting has still taken its bytes.
    pub(crate) async fn pace(&self, bytes: u64) {
        let now = Instant::now();
        let wait = {
            let mut full = self.full.lock().unwrap();
            *full = (*full).max(now) + self.time(bytes);
            full.saturating_duration_since(now).saturating_sub(self.time(BURST))
        };
        if !wait.is_zero() {
            tokio::time::sleep_until(now + wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::{Direction, Lane, MAX_CONNECTIONS, Pacer, Shape, TransferSettings};
    use crate::errors::Error;

    const PART: u64 = 512 << 10;

    #[test]
    fn a_user_account_keeps_its_shape_unpaced() {
        let settings = TransferSettings::default();
        let user = |direction, premium| (settings.shape(direction, false, premium), settings.rate(direction, false));
        assert_eq!(user(Direction::Download, false), (Shape::new(4, 1), None));
        assert_eq!(user(Direction::Upload, false), (Shape::new(4, 2), None));
        assert_eq!(user(Direction::Download, true), (Shape::new(8, 4), None));
        assert_eq!(user(Direction::Upload, true), (Shape::new(8, 1), None));
        // The bytes in flight before shapes: 2 MiB down, 16 MiB with Premium, 4 MiB up either way.
        let in_flight = |direction, premium| settings.shape(direction, false, premium).in_flight(PART);
        assert_eq!(in_flight(Direction::Download, false), 2 << 20);
        assert_eq!(in_flight(Direction::Download, true), 16 << 20);
        assert_eq!((in_flight(Direction::Upload, false), in_flight(Direction::Upload, true)), (4 << 20, 4 << 20));
    }

    #[test]
    fn a_bot_gets_8_connections_paced() {
        let settings = TransferSettings::default();
        assert_eq!(settings.shape(Direction::Download, true, false), Shape::new(8, 1));
        assert_eq!(settings.shape(Direction::Upload, true, false), Shape::new(8, 2));
        assert_eq!(settings.rate(Direction::Download, true), Some(20 << 20));
        assert_eq!(settings.rate(Direction::Upload, true), Some(19 * (1 << 20) + (1 << 19)));
    }

    #[test]
    fn overrides_replace_the_defaults_and_bad_ones_are_refused() {
        let lane = |connections, rate| Lane { connections, max_bytes_per_sec: rate };
        let settings = TransferSettings { download: lane(Some(2), Some(None)), upload: lane(None, Some(Some(1000))) };
        assert!(settings.validate().is_ok());
        assert_eq!(settings.shape(Direction::Download, true, false), Shape::new(2, 1));
        assert_eq!(settings.shape(Direction::Download, false, true), Shape::new(2, 4));
        assert_eq!(settings.rate(Direction::Download, true), None);
        assert_eq!(settings.shape(Direction::Upload, true, false), Shape::new(8, 2));
        let upload = (settings.rate(Direction::Upload, true), settings.rate(Direction::Upload, false));
        assert_eq!(upload, (Some(1000), Some(1000)));

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
}
