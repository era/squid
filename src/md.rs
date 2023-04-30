use anyhow::Result;
use gray_matter::engine::YAML;
use gray_matter::Matter;
use std::collections::HashMap;
use std::path::PathBuf;

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
}

#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub header: HashMap<String, String>,
    pub html_content: String,
    pub name: String,
}

impl MarkdownDocument {
    pub fn new(content: &str, name: String) -> Result<Self> {
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
        })
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

        let markdown = MarkdownDocument::new(content, "my_file.md".into()).unwrap();

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
