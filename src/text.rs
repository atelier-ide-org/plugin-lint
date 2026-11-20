//! Places in a text as the tools give them (bytes, characters or UTF-16 units, from 0 or 1) and
//! as the IDE reads them: 0-based lines and UTF-16 columns.

use atelier_plugin_api::protocol::{Diagnostic, Edit, Fix, Severity};

/// How a tool counts the columns of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Bytes,
    Chars,
    Utf16,
}

/// A text, with where its lines start.
pub struct Text<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Text<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, starts }
    }

    /// The line `line` (0-based), without its end; empty past the end of the text.
    pub fn line(&self, line: usize) -> &'a str {
        let Some(&start) = self.starts.get(line) else {
            return "";
        };
        let end = self.starts.get(line + 1).map_or(self.text.len(), |e| e - 1);
        self.text[start..end.max(start)].trim_end_matches('\r')
    }

    /// The UTF-16 column of the column `column` (0-based, in `unit`) of the line `line`.
    pub fn column(&self, line: usize, column: usize, unit: Unit) -> u32 {
        let text = self.line(line);
        let units: usize = match unit {
            Unit::Utf16 => column,
            Unit::Chars => text.chars().take(column).map(char::len_utf16).sum(),
            Unit::Bytes => {
                let mut end = column.min(text.len());
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text[..end].encode_utf16().count()
            }
        };
        units as u32
    }

    /// The 0-based line and the UTF-16 column of the byte `offset` of the text.
    pub fn position(&self, offset: usize) -> (u32, u32) {
        let offset = offset.min(self.text.len());
        let line = self.starts.partition_point(|&s| s <= offset) - 1;
        let column = self.column(line, offset - self.starts[line], Unit::Bytes);
        (line as u32, column)
    }

    /// The 0-based line and the UTF-16 column of the 1-based `line` and `column` in `unit`.
    pub fn at(&self, line: usize, column: usize, unit: Unit) -> (u32, u32) {
        let line = line.saturating_sub(1);
        (
            line as u32,
            self.column(line, column.saturating_sub(1), unit),
        )
    }

    /// The 0-based line and the UTF-16 column of the UTF-16 `offset` of the whole text, as
    /// JavaScript counts (ESLint's ranges).
    pub fn position_utf16(&self, offset: usize) -> (u32, u32) {
        let mut units = 0;
        for (byte, c) in self.text.char_indices() {
            if units >= offset {
                return self.position(byte);
            }
            units += c.len_utf16();
        }
        self.position(self.text.len())
    }

    /// The UTF-16 column of the end of the line `line`.
    pub fn line_end(&self, line: usize) -> u32 {
        self.line(line).encode_utf16().count() as u32
    }

    /// The UTF-16 column where the text of the line `line` starts, after its indentation.
    pub fn line_start(&self, line: usize) -> u32 {
        let text = self.line(line);
        (text.len() - text.trim_start().len()) as u32
    }
}

/// A problem from 1-based lines and columns (`end` exclusive) in `unit`, as most tools give
/// them. Without an end, or with an empty one, the word at the start is underlined, else the
/// line from there.
#[allow(clippy::too_many_arguments)]
pub fn problem(
    text: &Text,
    line: usize,
    column: usize,
    end: Option<(usize, usize)>,
    unit: Unit,
    severity: Severity,
    message: impl Into<String>,
    source: &str,
    code: Option<String>,
) -> Diagnostic {
    let line0 = line.saturating_sub(1);
    let mut start = text.column(line0, column.saturating_sub(1), unit);
    let (end_line, end_column) = match end {
        Some((l, c)) if (l, c) > (line, column) => {
            let l0 = l.saturating_sub(1);
            (l0 as u32, text.column(l0, c.saturating_sub(1), unit))
        }
        // At the end of the line, or past it (a missing semicolon…): the whole line.
        _ if start >= text.line_end(line0) => {
            start = text.line_start(line0).min(start);
            (line0 as u32, text.line_end(line0))
        }
        _ => (line0 as u32, word_end(text, line0, start)),
    };
    Diagnostic {
        line: line0 as u32,
        column: start,
        end_line,
        end_column,
        severity,
        message: message.into(),
        source: source.to_string(),
        code: code.filter(|c| !c.is_empty()),
        fixes: Vec::new(),
    }
}

/// A problem on a byte range of the text the tool read.
pub fn problem_at_bytes(
    text: &Text,
    range: std::ops::Range<usize>,
    severity: Severity,
    message: impl Into<String>,
    source: &str,
    code: Option<String>,
) -> Diagnostic {
    let (line, column) = text.position(range.start);
    let (end_line, mut end_column) = text.position(range.end.max(range.start));
    if (end_line, end_column) == (line, column) {
        end_column = word_end(text, line as usize, column);
    }
    Diagnostic {
        line,
        column,
        end_line,
        end_column,
        severity,
        message: message.into(),
        source: source.to_string(),
        code: code.filter(|c| !c.is_empty()),
        fixes: Vec::new(),
    }
}

/// Where the word at `column` (UTF-16) of the line `line` ends; the end of the line when there
/// is no word there, and the whole line when the column is past its text.
fn word_end(text: &Text, line: usize, column: u32) -> u32 {
    let chars: Vec<char> = text.line(line).chars().collect();
    let mut units = 0;
    let mut index = 0;
    while index < chars.len() && units < column {
        units += chars[index].len_utf16() as u32;
        index += 1;
    }
    let word = |c: &char| c.is_alphanumeric() || *c == '_' || *c == '$';
    if index >= chars.len() {
        return text.line_end(line);
    }
    if !word(&chars[index]) {
        return units + chars[index].len_utf16() as u32;
    }
    units
        + chars[index..]
            .iter()
            .take_while(|c| word(c))
            .map(|c| c.len_utf16() as u32)
            .sum::<u32>()
}

/// A change from `start` to `end` (0-based lines, UTF-16 columns).
pub fn edit(start: (u32, u32), end: (u32, u32), text: impl Into<String>) -> Edit {
    let end = end.max(start);
    Edit {
        line: start.0,
        column: start.1,
        end_line: end.0,
        end_column: end.1,
        text: text.into(),
    }
}

pub fn fix(title: impl Into<String>, edits: Vec<Edit>, preferred: bool) -> Fix {
    Fix {
        title: title.into(),
        edits,
        preferred,
    }
}

/// The text without the colors and styles of a terminal.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // Up to the final byte of the sequence (a letter).
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_in_each_unit_become_utf16() {
        // `é` is 2 bytes and 1 unit; `😀` is 4 bytes, 1 character and 2 units.
        let text = Text::new("é😀x\r\nnext");
        assert_eq!(text.line(0), "é😀x");
        assert_eq!(text.column(0, 6, Unit::Bytes), 3);
        assert_eq!(text.column(0, 2, Unit::Chars), 3);
        assert_eq!(text.column(0, 3, Unit::Utf16), 3);
        assert_eq!(text.position(9), (1, 0));
        assert_eq!(text.position(6), (0, 3));
        assert_eq!(text.line(5), "");
    }

    #[test]
    fn a_problem_without_an_end_underlines_the_word() {
        let text = Text::new("let unused = 2;\n");
        let p = problem(
            &text,
            1,
            5,
            None,
            Unit::Chars,
            Severity::Warning,
            "unused",
            "x",
            Some(String::new()),
        );
        assert_eq!((p.line, p.column, p.end_line, p.end_column), (0, 4, 0, 10));
        assert_eq!(p.code, None);
        let p = problem(
            &text,
            1,
            1,
            Some((1, 1)),
            Unit::Chars,
            Severity::Error,
            "m",
            "x",
            None,
        );
        assert_eq!(p.end_column, 3);
        // Past the text of the line: the line.
        let p = problem(
            &text,
            1,
            40,
            None,
            Unit::Chars,
            Severity::Error,
            "m",
            "x",
            None,
        );
        assert_eq!((p.column, p.end_column), (0, 15));
    }

    #[test]
    fn terminal_colors_are_removed() {
        assert_eq!(plain("\u{1b}[31merror\u{1b}[39m] x"), "error] x");
    }
}
