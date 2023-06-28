use crate::config::Configuration;
use crate::http;
use crate::io::copy_dir;
use crate::template::Website;
use crate::watch::FolderWatcher;
use clap::Parser;
use std::path::Path;
use std::process::exit;
use tokio::runtime::Handle;
use tokio::sync::mpsc;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Args {
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

    #[arg(short = 'p', long)]
    serve: Option<u16>,
}

pub struct App {
    args: Args,
}

impl App {
    pub fn new() -> Self {
        Self {
            args: Args::parse(),
        }
    }

    pub async fn run(&mut self) {
        self.build_website().await;

        let mut async_server = None;

        if let Some(port) = self.args.serve.as_ref() {
            println!("Serving website at http://127.0.0.1:{port}");
            let folder = &self.args.output_folder;
            async_server = Some(http::serve(*port, folder));
        }

        if let Some(async_server) = async_server {
            async_server.await.unwrap();
        } else if self.args.watch {
            println!("going to watch for change on files");
            let handle = Handle::current();
            self.watch(handle).await;
        }
    }

    async fn build_website(&self) {
        let template_folder = Path::new(&self.args.template_folder);
        let output_folder = Path::new(&self.args.output_folder);
        let config = self
            .args
            .template_variables
            .as_ref()
            .map(|f| Configuration::from_toml(f).unwrap());
        let markdown_folder = self
            .args
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

        let static_resources = self
            .args
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
    async fn watch(&self, handler: Handle) {
        let (tx, mut rx) = mpsc::channel(1);
        let mut watcher = FolderWatcher::new(handler, tx);

        watcher.watch(&self.args.template_folder).unwrap();

        if let Some(markdown_folder) = self.args.markdown_folder.as_ref() {
            watcher.watch(markdown_folder).unwrap();
        }

        if let Some(template_var) = self.args.template_variables.as_ref() {
            watcher.watch(template_var).unwrap();
        }

        while let Some(_m) = rx.recv().await {
            println!("Detected changes on files, rebuilding site");
            //TODO in the future only rebuild the parts that need to be rebuild
            self.build_website().await;
            println!("Site rebuilt");
        }
    }
}
