//! The short name of [vaegram](https://docs.rs/vaegram): everything in `vaegram` is here under `vae`.
//!
//! With the `macros` feature, give every `#[derive(BotCommands)]` enum `#[command(crate = "vae")]`. The derive
//! names the `vaegram` crate otherwise, which an app that depends only on `vae` can't reach.

pub use vaegram::*;
