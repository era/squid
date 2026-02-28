use crate::deps::FileChangeEvent;
use notify::{Error, Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::runtime::Handle;
use tokio::sync::mpsc::Sender;

pub struct FolderWatcher {
    handle: Handle,
    tx: Sender<FileChangeEvent>,
    watchers: Vec<RecommendedWatcher>,
}

impl FolderWatcher {
    pub fn new(handle: Handle, tx: Sender<FileChangeEvent>) -> Self {
        Self {
            tx,
            handle,
            watchers: Vec::new(),
        }
    }

    pub fn watch(
        &mut self,
        folder: &str,
        change_type: crate::deps::FileChangeType,
    ) -> Result<(), Error> {
        let event_handler =
            WatchEventHandler::new(self.handle.clone(), self.tx.clone(), change_type);

        let mut recommend_watcher =
            RecommendedWatcher::new(event_handler, notify::Config::default())?;
        recommend_watcher.watch(Path::new(folder), RecursiveMode::Recursive)?;
        self.watchers.push(recommend_watcher);

        Ok(())
    }
}

#[derive(Clone)]
pub struct WatchEventHandler {
    handler: Handle,
    change_type: crate::deps::FileChangeType,
    tx: Sender<FileChangeEvent>,
}

impl EventHandler for WatchEventHandler {
    fn handle_event(&mut self, event: notify::Result<Event>) {
        let tx = self.tx.clone();
        let change_type = self.change_type.clone();
        let paths = event.map(|e| e.paths).unwrap_or_default();
        self.handler.spawn(async move {
            if !paths.is_empty() {
                let _ = tx.send(FileChangeEvent { change_type, paths }).await;
            }
        });
    }
}

impl WatchEventHandler {
    pub fn new(
        handler: Handle,
        tx: Sender<FileChangeEvent>,
        change_type: crate::deps::FileChangeType,
    ) -> Self {
        Self {
            handler,
            tx,
            change_type,
        }
    }
}
