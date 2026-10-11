//! The log file: `mdk.log` in the config folder, rotated at 5 MB with one older file kept.
//!
//! It records what mdk did (start, folders connected, scan times, failures). It never
//! records what is in a card.

use std::fs::{self, OpenOptions};
use std::path::Path;

const MAX_BYTES: u64 = 5 * 1024 * 1024;

/// Start logging to `dir/mdk.log`. `MDK_LOG` sets the level (`error`, `warn`, `info`,
/// `debug`, `trace`); the default is `info`. A log that cannot be opened is not an error:
/// mdk runs without one.
pub fn init(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let path = dir.join("mdk.log");
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_BYTES) {
        let _ = fs::rename(&path, dir.join("mdk.log.1"));
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let level = std::env::var("MDK_LOG")
        .ok()
        .and_then(|level| level.parse().ok())
        .unwrap_or(log::LevelFilter::Info);
    let started = env_logger::Builder::new()
        .filter_level(level)
        .target(env_logger::Target::Pipe(Box::new(file)))
        .try_init();
    if started.is_err() {
        return;
    }
    // A panic goes to the log with where it happened, then to the default handler.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let trace = std::backtrace::Backtrace::force_capture();
        log::error!("panic: {panic}\n{trace}");
        default(panic);
    }));
    log::info!(
        "mdk {} started on {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}
