mod io;
mod template;

use crate::template::Website;
use clap::Parser;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    template_folder: String,

    #[arg(short, long)]
    partials_folder: Option<String>,

    #[arg(short, long)]
    output_folder: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let template_folder = Path::new(&args.template_folder);
    let output_folder = Path::new(&args.output_folder);

    let website = Website::new(template_folder.to_path_buf(), None);

    let mut files_processed = website.build(output_folder).unwrap();

    while let Some(res) = files_processed.join_next().await {
        match res {
            Ok(file) => {
                println!("successfully processed {file}");
            }
            Err(e) => eprintln!("task failed {e:?}"),
        };
    }
}
