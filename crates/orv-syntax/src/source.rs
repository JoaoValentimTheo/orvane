//! Source files and the source map that owns them.
//!
//! A [`SourceMap`] holds every file the compiler has read. [`FileId`] is a
//! stable, cheap handle into it, so spans never carry `String`s around.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::diagnostic::Diagnostic;

/// Index of a file inside a [`SourceMap`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct FileId(pub u32);

/// A decoded source file. The text is shared via [`Rc`] so diagnostics can
/// borrow it without cloning.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub id: FileId,
    /// Display name: usually the path as given on the command line.
    pub name: String,
    /// Path on disk, when the file came from disk (`<stdin>`/`<repl>` do not).
    pub path: Option<PathBuf>,
    /// Full UTF-8 contents, BOM stripped (SPEC §5.1).
    pub text: Rc<str>,
    /// Byte offset of the first character of every line, so [`Self::line_col`]
    /// is a binary search instead of a scan.
    ///
    /// Always starts with `0`. Built by [`Self::new`]; private so it cannot
    /// drift from `text`.
    line_starts: Rc<[u32]>,
}

impl SourceFile {
    /// Creates a source file, indexing its line starts.
    pub fn new(id: FileId, name: impl Into<String>, text: impl Into<String>) -> Self {
        let name = name.into();
        let text: Rc<str> = Rc::from(text.into());
        let line_starts = index_line_starts(&text);
        Self {
            id,
            name,
            path: None,
            text,
            line_starts,
        }
    }

    /// The text of the file.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Byte offset of the start of each line, with `line_starts[0] == 0`.
    pub fn line_starts(&self) -> &[u32] {
        &self.line_starts
    }

    /// Returns the 1-based line and column (in `char`s) of a byte offset.
    ///
    /// Out-of-range offsets are clamped to the end of the file, so this never
    /// panics on a malformed span. Costs a binary search plus the `char` count
    /// within one line, rather than a scan of the whole file.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = (offset as usize).min(self.text.len());
        // The last line start that is `<= offset` identifies the line.
        let line_index = match self.line_starts.binary_search(&(offset as u32)) {
            Ok(exact) => exact,
            // `Err(0)` cannot happen because `line_starts[0] == 0` and
            // `offset >= 0`, so the insertion point is always at least 1.
            Err(insertion) => insertion.saturating_sub(1),
        };
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0) as usize;
        let column = self.text[line_start..offset].chars().count() as u32 + 1;
        (line_index as u32 + 1, column)
    }

    /// The 1-based line of a byte offset.
    ///
    /// Used by golden `.err` output (§8.1), which anchors at the primary span
    /// start.
    pub fn line_of(&self, offset: u32) -> u32 {
        self.line_col(offset).0
    }
}

/// Collects the byte offset of the first character of every line.
///
/// A line break is `\n`; `\r\n` counts once, because the `\n` is the start of
/// nothing. A lone `\r` is not a break here (SPEC §5.1), keeping this in sync
/// with the lexer.
fn index_line_starts(text: &str) -> Rc<[u32]> {
    let mut starts = Vec::with_capacity(text.len() / 24 + 1);
    starts.push(0u32);
    for (idx, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(idx as u32 + 1);
        }
    }
    // A trailing newline would otherwise create an empty final line whose start
    // is `text.len()`, which is still a valid position to ask about.
    Rc::from(starts)
}

/// Owns every source file loaded so far.
#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// Creates an empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file with an explicit display name and returns its id.
    ///
    /// A UTF-8 BOM is stripped, as required by SPEC §5.1.
    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> FileId {
        let text = strip_bom(&text.into());
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile::new(id, name, text));
        id
    }

    /// Adds a file read from disk, keeping its path for diagnostics.
    pub fn load(&mut self, path: impl AsRef<Path>) -> std::io::Result<FileId> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)?;
        let name = path.display().to_string();
        let id = self.add(name, text);
        if let Some(file) = self.files.get_mut(id.0 as usize) {
            file.path = Some(path.to_path_buf());
        }
        Ok(id)
    }

    /// Looks up a file by id.
    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    /// Number of files in the map.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the map has no files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// All files, in insertion order (deterministic by construction).
    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// Renders `diag` with `ariadne` to stderr.
    ///
    /// # Errors
    /// Returns an [`std::io::Error`] if writing to stderr fails.
    pub fn render(&self, diag: &Diagnostic) -> std::io::Result<()> {
        crate::render::render(self, diag)
    }
}

fn strip_bom(text: &str) -> String {
    text.strip_prefix('\u{feff}').unwrap_or(text).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_assigns_sequential_ids() {
        let mut sm = SourceMap::new();
        let a = sm.add("a.orv", "fn main() {}");
        let b = sm.add("b.orv", "");
        assert_eq!(a, FileId(0));
        assert_eq!(b, FileId(1));
        assert_eq!(sm.len(), 2);
        assert!(!sm.is_empty());
    }

    #[test]
    fn add_strips_utf8_bom() {
        let mut sm = SourceMap::new();
        let id = sm.add("bom.orv", "\u{feff}print(1)");
        assert_eq!(sm.file(id).map(|f| f.text()), Some("print(1)"));
    }

    #[test]
    fn line_col_is_one_based() {
        let mut sm = SourceMap::new();
        let id = sm.add("x.orv", "ab\ncde\n");
        let f = sm.file(id);
        let f = f.expect("file exists");
        assert_eq!(f.line_col(0), (1, 1));
        assert_eq!(f.line_col(1), (1, 2));
        assert_eq!(f.line_col(3), (2, 1));
        assert_eq!(f.line_col(5), (2, 3));
    }

    #[test]
    fn line_col_clamps_out_of_range_offset() {
        let mut sm = SourceMap::new();
        let id = sm.add("x.orv", "ab");
        let f = sm.file(id).expect("file exists");
        assert_eq!(f.line_col(9_999), (1, 3));
    }

    #[test]
    fn line_col_counts_chars_not_bytes() {
        let mut sm = SourceMap::new();
        let id = sm.add("utf8.orv", "áéí");
        let f = sm.file(id).expect("file exists");
        assert_eq!(f.line_col(2), (1, 2));
        assert_eq!(f.line_col(6), (1, 4));
    }

    #[test]
    fn file_returns_none_for_unknown_id() {
        let sm = SourceMap::new();
        assert!(sm.file(FileId(7)).is_none());
    }

    #[test]
    fn files_preserve_insertion_order() {
        let mut sm = SourceMap::new();
        sm.add("first.orv", "");
        sm.add("second.orv", "");
        let names: Vec<&str> = sm.files().iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["first.orv", "second.orv"]);
    }

    #[test]
    fn line_starts_index_every_line() {
        let f = SourceFile::new(FileId(0), "x.orv", "ab\ncde\nf");
        // Lines start at bytes 0, 3 and 7.
        assert_eq!(f.line_starts(), &[0, 3, 7]);
    }

    #[test]
    fn line_starts_of_an_empty_file_is_just_zero() {
        let f = SourceFile::new(FileId(0), "x.orv", "");
        assert_eq!(f.line_starts(), &[0]);
        assert_eq!(f.line_col(0), (1, 1));
    }

    #[test]
    fn line_starts_treat_crlf_as_one_break() {
        // The `\n` is the break, so no start lands on the CR.
        let f = SourceFile::new(FileId(0), "x.orv", "a\r\nb\r\n");
        assert_eq!(f.line_starts(), &[0, 3, 6]);
        assert_eq!(f.line_col(3), (2, 1));
        assert_eq!(f.line_col(6), (3, 1));
    }

    #[test]
    fn line_starts_ignore_a_lone_carriage_return() {
        // A lone `\r` is whitespace (SPEC §5.1), so it starts no line.
        let f = SourceFile::new(FileId(0), "x.orv", "a\rb");
        assert_eq!(f.line_starts(), &[0]);
        assert_eq!(f.line_col(2), (1, 3));
    }

    #[test]
    fn line_col_matches_a_naive_scan_on_every_offset() {
        // The indexed lookup must agree with the obvious implementation.
        let text = "ab\ncde\n\nfgh\n\r\nijk\n";
        let f = SourceFile::new(FileId(0), "x.orv", text);
        for offset in 0..=text.len() as u32 {
            assert_eq!(
                f.line_col(offset),
                naive_line_col(text, offset),
                "mismatch at offset {offset}"
            );
        }
    }

    #[test]
    fn line_col_is_at_least_linear_only_once_per_file() {
        // Sanity check on the index: a many-line file has a start per line.
        let text = "x\n".repeat(1000);
        let f = SourceFile::new(FileId(0), "x.orv", &text);
        assert_eq!(f.line_starts().len(), 1001);
        assert_eq!(f.line_col(2000), (1001, 1));
    }

    /// The straightforward scan, kept in the test module as a reference.
    fn naive_line_col(text: &str, offset: u32) -> (u32, u32) {
        let offset = (offset as usize).min(text.len());
        let mut line = 1u32;
        let mut line_start = 0usize;
        for (idx, ch) in text.char_indices() {
            if idx >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                line_start = idx + ch.len_utf8();
            }
        }
        let column = text[line_start..offset].chars().count() as u32 + 1;
        (line, column)
    }
}
