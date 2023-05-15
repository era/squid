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
        } else if path.is_dir() {
            let sub_folder_name = path.file_name().unwrap().to_str().unwrap().to_owned();
            let sub_folder_contents = read_folder_contents(&path);
            for (file_name, file_contents) in sub_folder_contents {
                let prefixed_file_name = format!("{}/{}", sub_folder_name, file_name);
                contents.insert(prefixed_file_name, file_contents);
            }
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
        .arg("--markdown-folder")
        .arg("tests/markdown")
        .arg("--template-variables")
        .arg("tests/config.toml")
        .arg("--static-resources")
        .arg("tests/static")
        .assert()
        .success();

    let created = read_folder_contents(tempdir.path());
    let expected = read_folder_contents(&Path::new("tests/output/"));

    assert!(!created.is_empty());
    for (key, value) in created {
        let expected_content = match expected.get(&key) {
            Some(t) => t,
            None => {
                println!("{value}");
                panic!("we were not expecting {key}");
            }
        };
        assert_eq!(expected_content, &value);
    }
}
