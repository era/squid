use crate::config::Configuration;
use crate::io;
use crate::io::{LazyFolderReader, TemplateFile};
use anyhow::Context;
use anyhow::Result;

use crate::md::{MarkdownCollection, MarkdownDocument};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tinylang::eval;
use tinylang::types::{FuncArguments, State, TinyLangType};
use tokio::fs::{create_dir, File};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

#[derive(Debug, Clone)]
pub struct Website {
    template_folder: PathBuf,
    posts_folder: Option<PathBuf>,
    configuration: Option<Configuration>,
}

/// We need to transform all the information we build about the collections to the
/// template State, so that users can use them. For example, listing all the markdown
/// posts and linking to them. TODO: Tests
pub fn build_collection_state(collections: &HashMap<String, MarkdownCollection>) -> State {
    let mut state = State::new();

    for (key, collection) in collections {
        let mut collection_state = State::new();
        collection_state.insert(
            "size".to_string(),
            TinyLangType::Numeric(collection.collection.len() as f64),
        );

        let mut items_state = Vec::new();

        for item in &collection.collection {
            let mut item_state = State::new();
            for (header_key, header_value) in &item.header {
                item_state.insert(header_key.clone(), header_value.clone().into());
            }
            item_state.insert(
                "partial_uri".to_string(),
                format!(
                    "{}/{}.html",
                    collection.relative_path.to_string_lossy(),
                    item.name
                )
                    .into(),
            );
            items_state.push(TinyLangType::Object(item_state));
        }

        collection_state.insert("items".into(), TinyLangType::Vec(Arc::new(items_state)));

        state.insert(key.clone(), collection_state.into());
    }
    state
}

/// Build the generic State that will be passed to all partials and templates
/// this allow users to define speciall variables that they may want to use on their
/// template.
fn build_state(config: Option<Configuration>) -> State {
    let mut state = HashMap::default();

    if let Some(c) = config {
        state.insert("website_name".into(), c.website_name.into());
        state.insert("uri".into(), c.uri.into());
        for (key, value) in c.custom_keys {
            state.insert(key, value.into());
        }
    }

    state.insert(
        "render".into(),
        TinyLangType::Function(Arc::new(Box::new(render))),
    );
    state
}

impl Website {
    pub(crate) fn new(
        configuration: Option<Configuration>,
        template_folder: PathBuf,
        posts_folder: Option<PathBuf>,
    ) -> Self {
        Self {
            template_folder,
            posts_folder,
            configuration,
        }
    }

    async fn build_markdown(&self) -> Result<HashMap<String, MarkdownCollection>> {
        let mut collections = HashMap::new();
        let posts_folder = match &self.posts_folder {
            Some(p) => p,
            None => return Ok(collections),
        };
        let mut markdown_folder_reader = io::LazyFolderReader::new(posts_folder, "md")
            .context("could not create lazy folder reader for markdown folder")?;
        while let Some(file) = markdown_folder_reader.async_next().await {
            let file = match file {
                Ok(f) => f,
                Err(e) => {
                    //todo log lib
                    eprintln!("{}", e);
                    continue;
                }
            };
            let markdown_content = match MarkdownDocument::new(&file.contents, file.name) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}", e);
                    continue;
                }
            };
            let mut path = file.path;
            // remove the filename
            path.pop();

            //TODO avoid unwrap
            let path_as_string = path.file_name().unwrap().to_string_lossy();
            let collection = collections
                .entry(path_as_string.to_string())
                .or_insert(MarkdownCollection::new(path));
            collection.collection.push(markdown_content);
        }

        Ok(collections)
    }

    pub async fn build(&self, output: &Path) -> Result<JoinSet<String>> {
        let mut template_folder_reader =
            LazyFolderReader::new(&self.template_folder, "template")
                .context("could not create lazy folder reader for template folder")?;

        let collections = self.build_markdown().await?;

        let mut join_set = JoinSet::new();
        while let Some(file) = template_folder_reader.async_next().await {
            let output_folder = output.to_path_buf();

            let file = file.unwrap();

            // should handle _name.template differently
            // if there is a collection called "name" should create one file for each item in it
            // otherwise should skip it (could be a partial)
            if file.name.starts_with('_') {
                // removing the first _
                // and the .template from the end
                // this is safe because we filtered based on the extension name ('.template')
                let collection_name = &file.name[1..file.name.len() - 9];
                if let Some(collection) = collections.get(collection_name) {
                    self.build_collection(
                        collections.clone(),
                        collection.clone(),
                        file,
                        output,
                        &mut join_set,
                    );
                }
                continue;
            }

            self.build_template(collections.clone(), file, output_folder, &mut join_set);
        }

        Ok(join_set)
    }

    /// build a template without any markdown
    fn build_template(
        &self,
        collections: HashMap<String, MarkdownCollection>,
        file: TemplateFile,
        output_folder: PathBuf,
        join_set: &mut JoinSet<String>,
    ) {
        let configuration = self.configuration.clone();
        join_set.spawn(async move {
            let name = file.name.replace(".template", ".html");
            let output = {
                let mut state = build_state(configuration);
                // passes all the collections state as well so users can use it for
                // things like pagination
                state.extend(build_collection_state(&collections).into_iter());
                eval(&file.contents, state).unwrap()
            };

            let output_file = output_folder.join(&name);
            let mut file = File::create(output_file).await.unwrap();
            file.write_all(output.as_bytes()).await.unwrap();
            name
        });
    }

    /// builds a collection of markdown files using the appropriate template
    fn build_collection(
        &self,
        collections: HashMap<String, MarkdownCollection>,
        collection: MarkdownCollection,
        template: TemplateFile,
        output: &Path,
        join_set: &mut JoinSet<String>,
    ) {
        // we need for each item in the collection
        // to evaluate the template using its header and content
        for item in collection.collection {
            let collections = collections.clone();
            let configuration = self.configuration.clone();
            let collection_name = collection.relative_path.clone();

            let collection_name = collection_name
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let template = template.clone();
            let output_folder = output.to_path_buf();

            join_set.spawn(async move {
                let html = {
                    let mut state = build_state(configuration);

                    // we need to add the header of the markdown file to our state
                    // so templates can use it
                    for (key, value) in &item.header {
                        state.insert(format!("{collection_name}_{key}"), value.to_string().into());
                    }
                    state.insert(
                        format!("{collection_name}_content"),
                        item.html_content.into(),
                    );

                    // passes all the collections state as well so users can use it for
                    // things like pagination
                    state.extend(build_collection_state(&collections).into_iter());
                    eval(&template.contents, state).unwrap()
                };

                // we need to save our file following the markdown file and not the template
                let name = item.name.replace(".md", ".html");
                let output_folder = output_folder.join(&collection_name);

                if !output_folder.exists() {
                    //TODO ignoring the error for now because more than one green thread
                    // may try to create the dir
                    create_dir(&output_folder).await;
                }

                let output_file = output_folder.join(&name);
                let mut file = File::create(output_file).await.unwrap();
                file.write_all(html.as_bytes()).await.unwrap();
                name
            });
        }
    }
}

/// exposes render as a function in the template itself.
fn render(arguments: FuncArguments, state: &State) -> TinyLangType {
    if arguments.is_empty() {
        return TinyLangType::Nil;
    }

    let page = match arguments.first().unwrap() {
        TinyLangType::String(page) => page.as_str(),
        _ => return TinyLangType::Nil,
    };

    let result = match fs::read_to_string(page) {
        Ok(c) => eval(&c, state.clone()),
        Err(e) => return TinyLangType::String(e.to_string()),
    };

    match result {
        Ok(content) => TinyLangType::String(content),
        Err(e) => TinyLangType::String(e.to_string()),
    }
}

