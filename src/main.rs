mod io;
mod template;
mod config;

use crate::template::Website;
use clap::Parser;
use std::path::Path;
use std::process::exit;
use crate::config::Configuration;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    template_folder: String,

    #[arg(short, long)]
    partials_folder: Option<String>,

    #[arg(short, long)]
    configuration: Option<String>,

    #[arg(short, long)]
    output_folder: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let template_folder = Path::new(&args.template_folder);
    let output_folder = Path::new(&args.output_folder);
    let config = args.configuration.and_then(|f| {
        Some(Configuration::from_toml(&f).unwrap())
    });

    let website = Website::new(config, template_folder.to_path_buf(), None);

    let mut files_processed = website.build(output_folder).unwrap();

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
    if failed {
        exit(1);
    }
}
