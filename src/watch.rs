use notify::{Error, Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::runtime::Handle;
use tokio::sync::mpsc::Sender;

pub struct FolderWatcher {
    event_handler: WatchEventHandler,
    watchers: Vec<RecommendedWatcher>,
}

impl FolderWatcher {
    pub fn new(handler: Handle, tx: Sender<()>) -> Self {
        Self {
            event_handler: WatchEventHandler::new(handler, tx),
            watchers: Vec::new(),
        }
    }

    pub fn watch(&mut self, folder: &str) -> Result<(), Error> {
        let mut watcher =
            RecommendedWatcher::new(self.event_handler.clone(), notify::Config::default())?;
        watcher.watch(Path::new(folder), RecursiveMode::Recursive)?;
        self.watchers.push(watcher);
        Ok(())
    }
}

#[derive(Clone)]
struct WatchEventHandler {
    handler: Handle,
    tx: Sender<()>,
}

impl EventHandler for WatchEventHandler {
    fn handle_event(&mut self, _event: notify::Result<Event>) {
        let tx = self.tx.clone();
        self.handler.spawn(async move { tx.send(()).await });
    }
}
impl WatchEventHandler {
    pub fn new(handler: Handle, tx: Sender<()>) -> Self {
        Self { handler, tx }
    }
}
