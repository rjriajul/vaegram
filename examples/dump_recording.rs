//! Prints a recording made with `.record(path)` as JSON lines, one per message, to read or diff it:
//!
//! cargo run --example dump_recording --features json -- bot.rec

use vaegram::storage::recording;

fn main() -> std::io::Result<()> {
    let Some(path) = std::env::args_os().nth(1) else {
        eprintln!("usage: dump_recording <file.rec>");
        std::process::exit(2);
    };
    for record in recording::read(path.as_ref())? {
        println!("{}", record.to_json());
    }
    Ok(())
}
