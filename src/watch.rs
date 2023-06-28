use notify::{Error, Event, EventHandler, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::runtime::Handle;
use tokio::sync::mpsc::Sender;

pub struct FolderWatcher<T>
where
    T: Clone,
{
    handle: Handle,
    tx: Sender<T>,
    watchers: Vec<RecommendedWatcher>,
}

impl<T> FolderWatcher<T>
where
    T: Clone,
    WatchEventHandler<T>: EventHandler,
{
    pub fn new(handle: Handle, tx: Sender<T>) -> Self {
        Self {
            tx,
            handle,
            watchers: Vec::new(),
        }
    }

    pub fn watch(&mut self, folder: &str, variant_to_send: T) -> Result<(), Error> {
        let event_handler =
            WatchEventHandler::new(self.handle.clone(), self.tx.clone(), variant_to_send);

        let mut recommend_watcher =
            RecommendedWatcher::new(event_handler, notify::Config::default())?;
        recommend_watcher.watch(Path::new(folder), RecursiveMode::Recursive)?;
        self.watchers.push(recommend_watcher);

        Ok(())
    }
}

#[derive(Clone)]
pub struct WatchEventHandler<T>
where
    T: Clone,
{
    handler: Handle,
    variant: T,
    tx: Sender<T>,
}

impl<T> EventHandler for WatchEventHandler<T>
where
    T: Send + 'static + Clone + Sync,
{
    fn handle_event(&mut self, _event: notify::Result<Event>) {
        let tx = self.tx.clone();
        let value = self.variant.clone();
        self.handler.spawn(async move { tx.send(value).await });
    }
}
impl<T> WatchEventHandler<T>
where
    T: Clone,
{
    pub fn new(handler: Handle, tx: Sender<T>, variant: T) -> Self {
        Self {
            handler,
            tx,
            variant,
        }
    }
}
