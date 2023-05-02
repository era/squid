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
use tokio::fs::create_dir;
use tokio::task::JoinSet;

struct InnerState {
    tinylang_state: Arc<State>,
    output_folder: PathBuf,
    parser_tasks: JoinSet<String>,
}

impl InnerState {
    fn new(state: State, output_folder: PathBuf) -> Self {
        Self {
            tinylang_state: Arc::new(state),
            output_folder,
            parser_tasks: JoinSet::new(),
        }
    }

    /// build a template without any markdown
    fn eval_template_to_output_file(&mut self, file: TemplateFile) {
        let output_folder = self.output_folder.to_path_buf();
        let state = self.tinylang_state.clone();

        self.parser_tasks.spawn(async move {
            let file_name = file.name.replace(".template", ".html");
            let html = {
                let state = (*state).clone();

                eval(&file.contents, state).unwrap()
            };

            io::write_to_disk(output_folder, &file_name, html).await;

            file_name
        });
    }

    async fn mk_collection_dir(&mut self, collection: &MarkdownCollection) -> PathBuf {
        let collection_name = collection.relative_path.clone();
        let collection_name = collection_name
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let output_folder = self.output_folder.to_path_buf();
        let output_folder = output_folder.join(&collection_name);

        if !output_folder.exists() {
            create_dir(&output_folder).await.unwrap();
        }
        output_folder
    }

    /// builds a collection of markdown files using the appropriate template
    async fn eval_markdown_collection_to_output_file(&mut self, collection: MarkdownCollection, template: TemplateFile) {
        let output_folder = self.mk_collection_dir(&collection).await;

        // we need for each item in the collection
        // to evaluate the template using its header and content
        for item in collection.collection {
            let output_folder = output_folder.clone();

            let state = self.tinylang_state.clone();

            let template = template.clone();

            self.parser_tasks.spawn(async move {
                let html = {
                    let mut state = (*state).clone();

                    state.insert("content".into(), item.as_tinylang_state().into());

                    eval(&template.contents, state).unwrap()
                };

                // we need to save our file following the markdown file and not the template
                let file_name = item.name.replace(".md", ".html");

                io::write_to_disk(output_folder, &file_name, html).await;

                file_name
            });
        }
    }
}

pub struct Website {
    template_folder: PathBuf,
    posts_folder: Option<PathBuf>,
    configuration: Option<Configuration>,
    inner_state: Option<InnerState>,
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
            inner_state: None,
        }
    }

    pub async fn build(&mut self, output: &Path) -> Result<JoinSet<String>> {
        let mut template_folder_reader =
            LazyFolderReader::new(&self.template_folder, "template")
                .context("could not create lazy folder reader for template folder")?;

        let collections = self.build_markdown_collections().await?;

        let inner_state = InnerState::new(self.build_state(&collections), output.to_path_buf());

        self.inner_state = Some(inner_state);

        while let Some(file) = template_folder_reader.async_next().await {
            let file = file.unwrap();
            let inner_state = self.inner_state.as_mut().unwrap();

            // should handle _name.template differently
            // if there is a collection called "name" should create one file for each item in it
            // otherwise should skip it (could be a partial)
            if file.name.starts_with('_') {
                // removing the first _
                // and the .template from the end
                // this is safe because we filtered based on the extension name ('.template')
                let collection_name = &file.name[1..file.name.len() - 9];
                if let Some(collection) = collections.get(collection_name) {
                    inner_state.eval_markdown_collection_to_output_file(collection.clone(), file).await;
                }
                continue;
            }

            inner_state.eval_template_to_output_file(file);
        }

        Ok(self.inner_state.take().unwrap().parser_tasks)
    }

    async fn build_markdown_collections(&self) -> Result<HashMap<String, MarkdownCollection>> {
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

            let markdown_content = match MarkdownDocument::new(
                &file.contents,
                file.name,
                self.partial_uri(&file.path),
            ) {
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

    fn partial_uri(&self, path: &Path) -> String {
        path.to_string_lossy()
            .to_string()
            // we leave only the relative path after the `posts_folder` to avoid
            // creating a url with a local path (e.g. $HOME/my_site/posts)
            .replace(
                self.posts_folder
                    .as_ref()
                    .unwrap()
                    .to_string_lossy()
                    .as_ref(),
                "",
            )
            .replace("md", "html")
    }

    /// We need to transform all the information we build about the collections to the
    /// template State, so that users can use them. For example, listing all the markdown
    /// posts and linking to them.
    pub fn build_collection_state(
        &self,
        collections: &HashMap<String, MarkdownCollection>,
    ) -> State {
        let mut state = State::new();

        for (key, collection) in collections {
            state.insert(key.clone(), collection.as_tinylang_state().into());
        }
        state
    }

    /// Build the generic State that will be passed to all partials and templates
    /// this allow users to define special variables that they may want to use on their
    /// template.
    fn build_default_state(&self) -> State {
        let mut state = HashMap::default();

        if let Some(c) = self.configuration.as_ref() {
            state.insert("website_name".into(), c.website_name.clone().into());
            state.insert("uri".into(), c.uri.clone().into());
            for (key, value) in &c.custom_keys {
                state.insert(key.clone(), value.clone().into());
            }
        }

        state.insert("render".into(), TinyLangType::Function(Arc::new(render)));
        state
    }

    fn build_state(&self, collections: &HashMap<String, MarkdownCollection>) -> State {
        let mut state = self.build_default_state();
        // passes all the collections state as well so users can use it for
        // things like pagination
        state.extend(self.build_collection_state(collections).into_iter());

        state
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
