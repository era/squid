use assert_cmd::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempdir::TempDir;

fn read_folder_contents(folder_path: &Path) -> HashMap<String, String> {
    let mut contents = HashMap::new();

    let entries = fs::read_dir(folder_path).unwrap();
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_file() {
            let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
            let file_contents = fs::read_to_string(path).unwrap();
            contents.insert(file_name, file_contents);
        }
    }

    contents
}

#[test]
fn test_creates_basic_output() {
    let tempdir = TempDir::new("output").unwrap();

    Command::cargo_bin("squid")
        .unwrap()
        .arg("--template-folder")
        .arg("tests/templates")
        .arg("--output-folder")
        .arg(tempdir.path())
        .arg("--configuration")
        .arg("tests/config.toml")
        .assert()
        .success();

    let created = read_folder_contents(tempdir.path());
    let expected = read_folder_contents(&Path::new("tests/output/"));

    assert!(!created.is_empty());

    for (key, value) in created {
        assert_eq!(expected.get(&key).unwrap(), &value);
    }
}
