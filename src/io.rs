use anyhow::Context;
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub struct LazyFolderReader {
    files: Vec<PathBuf>,

}

#[derive(Debug)]
pub struct TemplateFile {
    pub name: String,
    pub contents: String,
}

pub struct LazyFolderReaderIterator<'a> {
    reader: &'a LazyFolderReader,
    current_position: usize,
}

impl Iterator for LazyFolderReaderIterator<'_> {
    type Item = Result<TemplateFile>;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.current_position == self.reader.files.len() {
            return None;
        }

        let name = self.reader.files.get(self.current_position)?;
        self.current_position += 1;

        let contents = match fs::read_to_string(name) {
            Ok(contents) => contents,
            Err(e) => return Some(Err(e).context("could not read file")),
        };

        Some(Ok(TemplateFile {
            name: name.file_name()?.to_str()?.to_string(),
            contents,
        }))
    }
}

impl Iterator for LazyFolderReader {
    type Item = Result<TemplateFile>;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.files.is_empty() {
            return None;
        }

        let current = self.files.pop().unwrap();

        let contents = match fs::read_to_string(&current) {
            Ok(contents) => contents,
            Err(e) => return Some(Err(e).context("could not read file")),
        };

        Some(Ok(TemplateFile {
            name: current.file_name()?.to_str()?.to_string(),
            contents,
        }))
    }
}

impl LazyFolderReader {
    pub fn new(dir: &Path) -> Result<Self> {
        let paths = fs::read_dir(dir).context("could not read the folder")?;

        // TODO handle sub-directories
        let files = paths
            .map(|path| {
                let path = path.unwrap();
                path.path()
            })
            .collect();

        Ok(Self {
            files,
        })
    }
}

impl<'a> IntoIterator for &'a LazyFolderReader {
    type Item = Result<TemplateFile>;
    type IntoIter = LazyFolderReaderIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            reader: &self,
            current_position: 0
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashSet;
    use std::fs::File;
    use std::hash::Hash;
    use std::io::Write;
    use tempdir::TempDir;

    fn create_random_template_files(temp_dir: &TempDir, num_files: usize) -> Vec<PathBuf> {
        let mut file_names = Vec::new();

        for i in 0..num_files {
            let file_path = temp_dir.path().join(format!("file{}.template", i));
            let mut file = File::create(&file_path).unwrap();
            write!(
                file,
                "This is file {}.",
                file_path.file_name().unwrap().to_str().unwrap()
            )
            .unwrap();
            file_names.push(file_path);
        }

        file_names
    }

    fn iters_equal_any_order<T: Eq + Hash>(
        mut i1: impl Iterator<Item = T>,
        i2: impl Iterator<Item = T>,
    ) -> bool {
        let set: HashSet<T> = i2.collect();
        i1.all(|x| set.contains(&x))
    }

    #[test]
    fn test_creates_reader_with_all_files() {
        let tempdir = TempDir::new("templates").unwrap();
        let files = create_random_template_files(&tempdir, 10);
        let reader = LazyFolderReader::new(tempdir.path()).unwrap();
        assert!(iters_equal_any_order(
            files.into_iter(),
            reader.files.into_iter()
        ));
    }

    #[test]
    fn test_reader_into_iter() {
        let tempdir = TempDir::new("templates").unwrap();
        create_random_template_files(&tempdir, 5);
        let reader = LazyFolderReader::new(tempdir.path()).unwrap();

        assert_eq!(5, reader.files.len());
        let mut checks = 0;
        for file in &reader {
            let file = file.unwrap();
            assert_eq!(format!("This is file {}.", file.name), file.contents);
            checks += 1;
        }
        assert_eq!(5, checks);
    }

    #[test]
    fn test_reader_iter() {
        let tempdir = TempDir::new("templates").unwrap();
        create_random_template_files(&tempdir, 5);
        let reader = LazyFolderReader::new(tempdir.path()).unwrap();

        assert_eq!(5, reader.files.len());
        let mut checks = 0;
        for file in reader {
            let file = file.unwrap();
            assert_eq!(format!("This is file {}.", file.name), file.contents);
            checks += 1;
        }
        assert_eq!(5, checks);
    }
}
