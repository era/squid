use crate::io;
use anyhow::Context;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tinylang::eval;
use tinylang::types::{FuncArguments, State, TinyLangTypes};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;
use crate::config::Configuration;

pub struct Website {
    template_folder: PathBuf,
    posts_folder: Option<PathBuf>,
    configuration: Option<Configuration>,
}

fn build_state(config: Option<Configuration>) -> HashMap<String, TinyLangTypes> {
    let mut state = HashMap::default();

    config.and_then(|c| {
        state.insert("website_name".into(), c.website_name.into());
        state.insert("uri".into(), c.uri.into());
        for (key, value) in c.custom_keys {
            state.insert(key, value.into());
        }
        Some(())
    });
    state.insert(
        "render".into(),
        TinyLangTypes::Function(Arc::new(Box::new(render))),
    );
    state
}

impl Website {
    pub(crate) fn new(configuration: Option<Configuration>, template_folder: PathBuf, posts_folder: Option<PathBuf>) -> Self {
        Self {
            template_folder,
            posts_folder,
            configuration
        }
    }

    pub async fn build(&self, output: &Path) -> Result<JoinSet<String>> {
        let mut lazy_folder_reader = io::LazyFolderReader::new(&self.template_folder, "template")
            .context("could not create lazy folder reader")?;

        // should first process all posts/collections of markdown files


        let mut join_set = JoinSet::new();
        while let Some(file) = lazy_folder_reader.async_next().await {

            // should handle _name.template differently
            // if there is a collection called "name" should create one file for each item in it
            // otherwise should skip it (could be a partial)
            let output_folder = output.to_path_buf();

            let configuration = self.configuration.clone();

            join_set.spawn(async move {
                let file = file.unwrap();
                let name = file.name;
                let name = name.replace(".template", ".html");
                let output = eval(&file.contents, build_state(configuration)).unwrap();
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
fn render(arguments: FuncArguments, state: &State) -> TinyLangTypes {
    if arguments.is_empty() {
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
