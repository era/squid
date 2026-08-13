//! Rendering of Typst documents to HTML.
//!
//! Typst's html export (a full `<html>` document with its own `<head>`
//! styles) is compiled in-process through the `typst` crate, then reduced to
//! a fragment suitable for embedding into squid templates: the `<style>`
//! blocks from the head (needed for math and layout) plus the body content.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use anyhow::{anyhow, Context, Result};
use typst::diag::{FileError, FileResult, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook, FontInfo};
use typst::utils::LazyHash;
use typst::{Feature, Features, Library, LibraryExt, World};
use typst_html::{HtmlDocument, HtmlOptions};

/// Shared, expensive-to-build resources: the standard library with html
/// export enabled and the system font book. Built once per process; every
/// document compile reuses it.
///
/// Font files are only metadata-parsed up front ([`FontInfo`], cheap); the
/// full [`Font`] objects are constructed lazily on first use, mirroring what
/// the typst CLI does.
struct TypstEnv {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    /// Raw font data and collection index for each face in the book, in book
    /// order.
    faces: Vec<(Vec<u8>, u32)>,
    font_cache: Mutex<HashMap<usize, Font>>,
}

static ENV: LazyLock<TypstEnv> = LazyLock::new(|| {
    let library = Library::builder()
        .with_features(Features::from_iter([Feature::Html]))
        .build();

    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let mut book = FontBook::new();
    let mut faces = Vec::new();
    for face in db.faces() {
        let data = match &face.source {
            fontdb::Source::File(path) => std::fs::read(path).ok(),
            fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
        };
        let Some(data) = data else { continue };
        let Some(info) = FontInfo::new(&data, face.index) else {
            continue;
        };
        book.push(info);
        faces.push((data, face.index));
    }

    TypstEnv {
        library: LazyHash::new(library),
        book: LazyHash::new(book),
        faces,
        font_cache: Mutex::new(HashMap::new()),
    }
});

/// A minimal [`World`] compiling a single in-memory source file, backed by
/// the shared environment. `#include` and image files are not supported.
struct SingleFileWorld {
    env: &'static TypstEnv,
    main: FileId,
    source: Source,
}

impl World for SingleFileWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.env.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.env.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::NotFound(PathBuf::from(
                id.vpath().get_with_slash(),
            )))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(PathBuf::from(
            id.vpath().get_with_slash(),
        )))
    }

    fn font(&self, index: usize) -> Option<Font> {
        let mut cache = self
            .env
            .font_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(font) = cache.get(&index) {
            return Some(font.clone());
        }
        let (data, face_index) = self.env.faces.get(index)?;
        let font = Font::new(Bytes::new(data.clone()), *face_index)?;
        cache.insert(index, font.clone());
        Some(font)
    }

    fn today(&self, offset: Option<Duration>) -> Option<Datetime> {
        today(offset)
    }
}

/// The current date, honoring an explicit UTC offset. Mirrors what the typst
/// CLI does: without an offset the local date is used, with an offset the
/// date in that UTC offset.
fn today(offset: Option<Duration>) -> Option<Datetime> {
    use chrono::Datelike;

    let now_utc = chrono::Utc::now();
    let now = if offset.is_some() {
        now_utc.fixed_offset()
    } else {
        now_utc.with_timezone(&chrono::Local).fixed_offset()
    };

    let with_offset = match offset {
        None => now,
        Some(offset) => {
            let seconds = offset.seconds().trunc();
            if !seconds.is_finite()
                || seconds < f64::from(i32::MIN)
                || seconds > f64::from(i32::MAX)
            {
                return None;
            }
            now.with_timezone(&chrono::FixedOffset::east_opt(seconds as i32)?)
        }
    };

    Datetime::from_ymd(
        with_offset.year(),
        with_offset.month().try_into().ok()?,
        with_offset.day().try_into().ok()?,
    )
}

/// Formats typst diagnostics (compile errors or export errors) as one
/// message string.
fn render_diags(diags: &[SourceDiagnostic]) -> String {
    diags.iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compiles a Typst source document to an HTML fragment suitable for
/// embedding into a squid template.
pub async fn compile(source: &str) -> Result<String> {
    let vpath = VirtualPath::new("main.typ")
        .map_err(|e| anyhow!("could not create typst virtual path: {e}"))?;
    let main = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
    let world = SingleFileWorld {
        env: &ENV,
        main,
        source: Source::new(main, source.to_string()),
    };

    tokio::task::spawn_blocking(move || {
        let warned = typst::compile::<HtmlDocument>(&world);
        for warning in warned.warnings {
            eprintln!("typst warning: {}", warning.message);
        }
        let document = warned
            .output
            .map_err(|diags| anyhow!("typst compilation failed:\n{}", render_diags(&diags)))?;
        let full_html = typst_html::html(&document, &HtmlOptions { pretty: false })
            .map_err(|diags| anyhow!("typst html export failed: {}", render_diags(&diags)))?;
        Ok(extract_fragment(&full_html))
    })
    .await
    .context("typst compilation task failed")?
}

/// Typst's html export produces a full document (`<!DOCTYPE html>` with
/// `<head>` and `<body>`). Squid embeds the body content into its own
/// templates, so we keep the `<style>` blocks from the head and the inner
/// html of the body.
fn extract_fragment(full_html: &str) -> String {
    let mut fragment = String::new();

    // <style>...</style> blocks from the head
    let mut rest = full_html;
    while let Some(start) = rest.find("<style") {
        let Some(open_end) = rest[start..].find('>') else {
            break;
        };
        let style_start = start + open_end + 1;
        let Some(close) = rest[style_start..].find("</style>") else {
            break;
        };
        let close_end = style_start + close + "</style>".len();
        fragment.push_str(&rest[start..close_end]);
        rest = &rest[close_end..];
    }

    // inner content of <body ...> ... </body>
    if let Some(body_start) = full_html.find("<body") {
        if let Some(open_end) = full_html[body_start..].find('>') {
            let content_start = body_start + open_end + 1;
            if let Some(body_end) = full_html[content_start..].find("</body>") {
                fragment.push_str(&full_html[content_start..content_start + body_end]);
            }
        }
    }

    fragment
}

#[cfg(test)]
mod tests {
    use super::{compile, extract_fragment};

    #[tokio::test]
    async fn test_compile_renders_heading_and_markup() {
        let html = compile("= Hello\n\nThis is *typst* content.\n")
            .await
            .unwrap();
        assert_eq!(
            html,
            "<h2>Hello</h2><p>This is <strong>typst</strong> content.</p>"
        );
    }

    #[tokio::test]
    async fn test_compile_reports_diagnostics_on_error() {
        let err = compile("#unknown-function()").await.unwrap_err();
        assert!(err.to_string().contains("typst compilation failed"));
    }

    #[test]
    fn test_extract_fragment_keeps_head_styles_and_body() {
        let html = concat!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"/>",
            "<style>.typ { color: red }</style></head>",
            "<body><h1>Hello</h1><p>World</p></body></html>"
        );
        assert_eq!(
            extract_fragment(html),
            "<style>.typ { color: red }</style><h1>Hello</h1><p>World</p>"
        );
    }

    #[test]
    fn test_extract_fragment_without_styles() {
        let html = "<!DOCTYPE html><html><head><title>x</title></head><body><p>Hi</p></body></html>";
        assert_eq!(extract_fragment(html), "<p>Hi</p>");
    }

    #[test]
    fn test_extract_fragment_multiple_styles() {
        let html = concat!(
            "<!DOCTYPE html><html><head>",
            "<style>a{}</style><style>b{}</style>",
            "</head><body><div>c</div></body></html>"
        );
        assert_eq!(
            extract_fragment(html),
            "<style>a{}</style><style>b{}</style><div>c</div>"
        );
    }
}
