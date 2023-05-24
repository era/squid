mod config;
mod io;
mod md;
mod template;
mod tinylang;
mod watch;

use crate::config::Configuration;
use crate::io::copy_dir;
use crate::template::Website;
use clap::Parser;
use notify::{Event, EventHandler};
use std::path::Path;
use std::process::exit;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    template_folder: String,

    #[arg(short, long)]
    markdown_folder: Option<String>,

    #[arg(short, long)]
    static_resources: Option<String>,

    #[arg(short = 'v', long)]
    template_variables: Option<String>,

    #[arg(short, long)]
    output_folder: String,

    #[arg(short, long)]
    watch: bool,
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

#[tokio::main]
async fn main() {
    let args = Args::parse();

    build_website(&args).await;

    if args.watch {
        println!("going to watch for change on files");
        let handle = Handle::current();
        watch(args, handle).await;
    }
}

async fn build_website(args: &Args) {
    let template_folder = Path::new(&args.template_folder);
    let output_folder = Path::new(&args.output_folder);
    let config = args
        .template_variables
        .as_ref()
        .map(|f| Configuration::from_toml(f).unwrap());
    let markdown_folder = args
        .markdown_folder
        .as_ref()
        .map(|f| Path::new(&f).to_path_buf());

    let mut website = Website::new(config, template_folder.to_path_buf(), markdown_folder);
    let mut files_processed = website.build(output_folder).await.unwrap();

    let mut failed = false;

    while let Some(res) = files_processed.join_next().await {
        match res {
            Ok(file) => {
                println!("successfully processed {file}");
            }
            Err(e) => {
                eprintln!("task failed {e:?}");
                failed = true;
            }
        };
    }

    let static_resources = args
        .static_resources
        .as_ref()
        .map(|dir| copy_dir(Path::new(&dir), output_folder));

    if failed {
        exit(1);
    }

    match static_resources {
        Some(Err(e)) => {
            eprintln!(
                "task failed, could not copy static resources {:?}",
                e.to_string()
            );
            exit(1);
        }
        Some(_) => println!("Copied static resources"),
        _ => println!("No static resources to be copied over"),
    }
}

/// watches for change in the directories selected by the user
/// in order to re-build the website
async fn watch(args: Args, handler: Handle) {
    let (tx, mut rx) = mpsc::channel(1);
    let event_handler = WatchEventHandler { handler, tx };
    let mut watchers = Vec::with_capacity(3);

    watchers.push(watch::watch(&args.template_folder, event_handler.clone()).unwrap());

    if let Some(markdown_folder) = args.markdown_folder.as_ref() {
        watchers.push(watch::watch(markdown_folder, event_handler.clone()).unwrap());
    }

    if let Some(template_var) = args.template_variables.as_ref() {
        watchers.push(watch::watch(template_var, event_handler.clone()).unwrap());
    }

    while let Some(_m) = rx.recv().await {
        //TODO in the future only rebuild the parts that need to be rebuild
        build_website(&args).await
    }
}
