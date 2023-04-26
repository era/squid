use std::collections::HashMap;
use std::fs::read_to_string;
use serde::Serialize;
use serde::Deserialize;
use anyhow::{Context, Result};

#[derive(Serialize, Deserialize, Clone)]
pub struct Configuration {
    pub website_name: String,
    pub uri: String,
    pub custom_keys: HashMap<String, String>
}

impl Configuration {
    pub fn from_toml(path: &str) -> Result<Self> {
        let content = read_to_string(path).context("could not read config file")?;
        let configuration = toml::from_str(&content).context("invalid toml file")?;
        Ok(configuration)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use tempdir::TempDir;
    use std::io::Write;
    use super::*;

    #[test]
    fn test_from_toml() {
        let content = r#"
        website_name = "my website"
        uri = "https://my_website.com"
        [custom_keys]
        something = "nice"
        "#;
        let tempdir = TempDir::new("toml").unwrap();
        let file_path = tempdir.into_path().join("config.toml");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "{}", content);

        let config = Configuration::from_toml(file_path.to_str().unwrap()).unwrap();
        assert_eq!("my website".to_string(), config.website_name);
        assert_eq!("https://my_website.com".to_string(), config.uri);
        assert_eq!("nice", config.custom_keys.get("something").unwrap().as_str());

    }
}