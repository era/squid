use assert_cmd::Command;
use criterion::{criterion_group, criterion_main, Criterion};
use tempdir::TempDir;

pub fn criterion_benchmark(c: &mut Criterion) {
    let tempdir = TempDir::new("output").unwrap();
    c.bench_function("cli", |b| {
        b.iter(|| {
            Command::cargo_bin("squid")
                .unwrap()
                .arg("--template-folder")
                .arg("tests/templates")
                .arg("--output-folder")
                .arg(tempdir.path())
                .arg("--markdown-folder")
                .arg("tests/markdown")
                .arg("--configuration")
                .arg("tests/config.toml")
                .assert()
                .success();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
