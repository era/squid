use crate::io;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tinylang::eval;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

pub fn process_folder(input: &Path, output: &Path) -> Result<JoinSet<String>> {
    let lazy_folder_reader =
        io::LazyFolderReader::new(input).context("could not create lazy folder reader")?;

    let mut join_set = JoinSet::new();

    for file in lazy_folder_reader {
        let output_folder = output.to_path_buf();
        join_set.spawn(async move {
            let file = file.unwrap();
            let name = file.name;
            let output = eval(&file.contents, HashMap::default()).unwrap();
            let output_file = output_folder.join(&name);
            let mut file = File::create(output_file).await.unwrap();
            file.write_all(output.as_bytes()).await.unwrap();
            name
        });
    }

    Ok(join_set)
}
