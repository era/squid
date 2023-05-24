use notify::{Error, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub fn watch<F: EventHandler>(folder: &str, event_handler: F) -> Result<RecommendedWatcher, Error> {
    let mut watcher = RecommendedWatcher::new(event_handler, notify::Config::default())?;
    watcher.watch(Path::new(folder), RecursiveMode::Recursive)?;

    Ok(watcher)
}
