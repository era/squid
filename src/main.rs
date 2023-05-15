mod config;
mod io;
mod md;
mod template;
mod tinylang;

use crate::config::Configuration;
use crate::io::copy_dir;
use crate::template::Website;
use clap::Parser;
use std::path::Path;
use std::process::exit;

#[derive(Parser, Debug)]
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
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let template_folder = Path::new(&args.template_folder);
    let output_folder = Path::new(&args.output_folder);
    let config = args
        .template_variables
        .map(|f| Configuration::from_toml(&f).unwrap());
    let markdown_folder = args.markdown_folder.map(|f| Path::new(&f).to_path_buf());

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
