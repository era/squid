use assert_cmd::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
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
    let expected = read_folder_contents(Path::new("tests/output/"));

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

#[test]
fn test_watches() {
    let tempdir = TempDir::new("output").unwrap();
    let static_folder = TempDir::new("static_folder").unwrap();

    let output_folder = tempdir.path().to_str().to_owned().unwrap().to_string();
    let static_folder_cmd = static_folder
        .path()
        .to_str()
        .to_owned()
        .unwrap()
        .to_string();

    //TODO stop leaking thread
    std::thread::spawn(move || {
        println!("aaa");
        Command::cargo_bin("squid")
            .unwrap()
            .arg("--template-folder")
            .arg("tests/templates")
            .arg("--output-folder")
            .arg(output_folder)
            .arg("--markdown-folder")
            .arg("tests/markdown")
            .arg("--template-variables")
            .arg("tests/config.toml")
            .arg("--static-resources")
            .arg(static_folder_cmd)
            .arg("--watch")
            .unwrap()
            .unwrap();
    });

    File::create(static_folder.into_path().join("hello.txt")).unwrap();
    //TODO avoid sleeping during tests
    sleep(Duration::from_millis(100));
    assert!(tempdir.into_path().join("hello.txt").exists())
}
