use crate::io;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tinylang::eval;
use tinylang::types::{FuncArguments, TinyLangTypes};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

pub struct Website {
    template_folder: PathBuf,
    posts_folder: Option<PathBuf>,
}

type PartialsCache = HashMap<String, String>;

fn build_state() -> HashMap<String, TinyLangTypes> {
    let mut state = HashMap::default();
    state.insert(
        "render".into(),
        TinyLangTypes::Function(Arc::new(Box::new(render))),
    );
    state
}

impl Website {
    pub(crate) fn new(template_folder: PathBuf, posts_folder: Option<PathBuf>) -> Self {
        Self {
            template_folder,
            posts_folder,
        }
    }

    pub fn build(&self, output: &Path) -> Result<JoinSet<String>> {
        let lazy_folder_reader = io::LazyFolderReader::new(&self.template_folder, "template")
            .context("could not create lazy folder reader")?;

        let mut join_set = JoinSet::new();

        for file in lazy_folder_reader {
            let output_folder = output.to_path_buf();
            join_set.spawn(async move {
                let file = file.unwrap();
                let name = file.name;
                let name = name.replace(".template", ".html");
                let output = eval(&file.contents, build_state()).unwrap();
                let output_file = output_folder.join(&name);
                let mut file = File::create(output_file).await.unwrap();
                file.write_all(output.as_bytes()).await.unwrap();
                name
            });
        }

        Ok(join_set)
    }
}

/// exposes render as a function in the template itself.
fn render(arguments: FuncArguments, state: &HashMap<String, TinyLangTypes>) -> TinyLangTypes {
    if arguments.len() == 0 {
        return TinyLangTypes::Nil;
    }

    let page = match arguments.first().unwrap() {
        TinyLangTypes::String(page) => page.as_str(),
        _ => return TinyLangTypes::Nil,
    };

    let result = match fs::read_to_string(page) {
        Ok(c) => eval(&c, state.clone()),
        Err(e) => return TinyLangTypes::String(e.to_string()),
    };

    match result {
        Ok(content) => TinyLangTypes::String(content),
        Err(e) => TinyLangTypes::String(e.to_string()),
    }
}
