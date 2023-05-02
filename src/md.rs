use anyhow::Result;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tinylang::types::{State, TinyLangType};

#[derive(Debug, Clone)]
pub struct MarkdownCollection {
    pub name: String,
    pub relative_path: PathBuf,
    pub collection: Vec<MarkdownDocument>,
}

impl MarkdownCollection {
    pub fn new(path: PathBuf) -> Self {
        Self {
            name: path.to_str().unwrap().to_string(),
            relative_path: path,
            collection: Vec::new(),
        }
    }

    /// collects metadata about the collection and exposes it as TinyLang::State so
    /// it can be used on the templates we are building
    pub fn as_tinylang_state(&self) -> State {
        let mut collection_state = State::new();
        collection_state.insert(
            "size".to_string(),
            TinyLangType::Numeric(self.collection.len() as f64),
        );

        let mut items_state = Vec::new();

        for item in &self.collection {
            items_state.push(TinyLangType::Object(item.as_tinylang_state()));
        }

        collection_state.insert("items".into(), TinyLangType::Vec(Arc::new(items_state)));
        collection_state
    }
}

#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub header: HashMap<String, String>,
    pub html_content: String,
    pub name: String,
    pub partial_uri: String,
}

impl MarkdownDocument {
    pub fn new(content: &str, name: String, partial_uri: String) -> Result<Self> {
        let matter = Matter::<YAML>::new();
        let header = matter.parse(content);
        let html_content = markdown::to_html(&header.content);

        let header: HashMap<String, String> = match header.data {
            Some(d) => d.deserialize()?,
            None => HashMap::new(),
        };

        Ok(Self {
            header,
            html_content,
            name,
            partial_uri,
        })
    }

    /// collects metadata about the markdown document and exposes it as TinyLang::State so
    /// it can be used on the templates we are building
    pub fn as_tinylang_state(&self) -> State {
        let mut item_state = State::new();
        for (header_key, header_value) in &self.header {
            item_state.insert(header_key.clone(), header_value.clone().into());
        }

        item_state.insert("content".into(), self.html_content.clone().into());

        item_state.insert("partial_uri".to_string(), self.partial_uri.clone().into());
        item_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_markdown_document() {
        let content = r#"---
title: This is pretty cool
---
# This is an amazing content!"#;

        let markdown =
            MarkdownDocument::new(content, "my_file.md".into(), "/posts/".to_string()).unwrap();

        assert_eq!(
            "<h1>This is an amazing content!</h1>",
            markdown.html_content
        );

        assert_eq!(
            "This is pretty cool",
            markdown.header.get("title").unwrap().as_str()
        );
    }
}
