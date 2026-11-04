//! Ready-made request observers: pass one to [`ClientBuilder::observer`](crate::ClientBuilder::observer).

#[cfg(feature = "metrics")]
mod metrics;

#[cfg(feature = "metrics")]
pub use self::metrics::metrics;
