//! The Typst world: the one source file, the bundled fonts and the assets. Nothing else exists in it.

use std::sync::LazyLock;

use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// The bundled fonts, parsed once.
static FONTS: LazyLock<Vec<Font>> =
    LazyLock::new(|| crate::fonts::bundled().iter().flat_map(|f| Font::iter(Bytes::new(f.ttf))).collect());

/// The standard library, built once. No inputs, no features.
static LIBRARY: LazyLock<LazyHash<Library>> = LazyLock::new(|| LazyHash::new(Library::builder().build()));

/// The path of the main source file.
pub const MAIN: &str = "/main.typ";

fn id(path: &str) -> FileId {
    // The paths are made by the writer (`MAIN`, `/a<n>.<ext>`), so they are always valid.
    RootedPath::new(VirtualRoot::Project, VirtualPath::new(path).expect("a valid virtual path")).intern()
}

pub struct OrdWorld {
    main: Source,
    files: Vec<(FileId, Bytes)>,
    date: Option<Datetime>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl OrdWorld {
    /// A world of one source, the given virtual files (path to bytes), the document's own font files (after
    /// the bundled fonts) and a date for `today`.
    pub fn new(
        source: String,
        files: Vec<(String, Vec<u8>)>,
        extra_fonts: Vec<Vec<u8>>,
        date: Option<(i32, u8, u8)>,
    ) -> Self {
        let mut fonts = FONTS.clone();
        for f in extra_fonts {
            fonts.extend(Font::iter(Bytes::new(f)));
        }
        OrdWorld {
            main: Source::new(id(MAIN), source),
            files: files.into_iter().map(|(p, b)| (id(&p), Bytes::new(b))).collect(),
            date: date.and_then(|(y, m, d)| Datetime::from_ymd(y, m, d)),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
        }
    }

    /// True when the world has a font of this family (any case).
    pub fn has_family(&self, family: &str) -> bool {
        self.book.contains_family(&family.to_lowercase())
    }
}

impl World for OrdWorld {
    fn library(&self) -> &LazyHash<Library> {
        &LIBRARY
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() { Ok(self.main.clone()) } else { Err(FileError::AccessDenied) }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main.id() {
            return Ok(Bytes::from_string(self.main.clone()));
        }
        self.files.iter().find(|(f, _)| *f == id).map(|(_, b)| b.clone()).ok_or(FileError::AccessDenied)
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    /// The document date: the world has no clock.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        self.date
    }
}
