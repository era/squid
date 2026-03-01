use criterion::{black_box, criterion_group, criterion_main, Criterion};
use squid::{Configuration, MarkdownDocument, Website};
use std::path::Path;
use std::sync::Arc;
use tempdir::TempDir;
use tokio::runtime::Runtime;
use tokio::task::JoinSet;

const MARKDOWN_SAMPLE: &str = r#"---
title: This is such a nice title
date: 2025-05-12
tags: rust, blogging
excerpt: A sample excerpt
---
# This is my content

Some markdown with **bold** and *italic* text.
"#;

fn config_parse(c: &mut Criterion) {
    c.bench_function("config_parse", |b| {
        b.iter(|| {
            black_box(Configuration::from_toml("tests/config.toml").expect("config parse failed"))
        })
    });
}

fn markdown_parse_single(c: &mut Criterion) {
    c.bench_function("markdown_parse_single", |b| {
        b.iter(|| {
            black_box(
                MarkdownDocument::new(
                    MARKDOWN_SAMPLE,
                    "post.md".to_string(),
                    "/posts/post".to_string(),
                )
                .expect("markdown parse failed"),
            )
        })
    });
}

fn markdown_parse_single_large(c: &mut Criterion) {
    // Simulate a larger document
    let mut content = String::from(MARKDOWN_SAMPLE);
    for i in 0..50 {
        content.push_str(&format!("\n\n## Section {}\n\n", i));
        content.push_str("Lorem ipsum dolor sit amet, consectetur adipiscing elit. ");
        content.push_str("Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n");
    }

    c.bench_function("markdown_parse_single_large", |b| {
        b.iter(|| {
            black_box(
                MarkdownDocument::new(
                    &content,
                    "large_post.md".to_string(),
                    "/posts/large_post".to_string(),
                )
                .expect("markdown parse failed"),
            )
        })
    });
}

fn full_build(c: &mut Criterion) {
    let rt = Runtime::new().expect("runtime");
    let template_folder = Path::new("tests/templates").to_path_buf();
    let markdown_folder = Some(Path::new("tests/markdown").to_path_buf());
    let config = Configuration::from_toml("tests/config.toml").expect("config");

    c.bench_function("full_build", |b| {
        b.iter(|| {
            let tempdir = TempDir::new("bench_output").expect("tempdir");
            let output_path = tempdir.path().to_path_buf();

            let mut website = Website::new(
                Some(config.clone()),
                template_folder.clone(),
                markdown_folder.clone(),
            );

            let join_set = rt
                .block_on(website.build_from_scratch(&output_path))
                .expect("build failed");

            // Drain the JoinSet to ensure all template tasks complete
            rt.block_on(async {
                let mut set = join_set;
                while set.join_next().await.is_some() {}
            });
        })
    });
}

fn full_build_without_markdown(c: &mut Criterion) {
    let rt = Runtime::new().expect("runtime");
    let template_folder = Path::new("tests/templates").to_path_buf();
    let config = Configuration::from_toml("tests/config.toml").expect("config");

    c.bench_function("full_build_without_markdown", |b| {
        b.iter(|| {
            let tempdir = TempDir::new("bench_output").expect("tempdir");
            let output_path = tempdir.path().to_path_buf();

            let mut website = Website::new(Some(config.clone()), template_folder.clone(), None);

            let join_set = rt
                .block_on(website.build_from_scratch(&output_path))
                .expect("build failed");

            rt.block_on(async {
                let mut set = join_set;
                while set.join_next().await.is_some() {}
            });
        })
    });
}

criterion_group!(
    benches,
    config_parse,
    markdown_parse_single,
    markdown_parse_single_large,
    full_build,
    full_build_without_markdown
);
criterion_main!(benches);
