use crate::utils::{get_utf_index, prefix_by_width, suffix_by_width};
use std::borrow::Cow;
use std::fmt::Write;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use vte::{Params, Parser, Perform};

/// Plain lines share their display and clipboard text. Formatted lines also retain the input.
#[derive(Clone)]
pub enum Line {
    Clean(String),
    Ansi { raw: String, stripped: String },
}

impl Line {
    /// Returns the display/stripped version of the line (without ANSI codes).
    /// Cursor positions, regex matches, and clipboard regions refer to this text.
    pub fn display(&self) -> &str {
        match self {
            Line::Clean(s) => s,
            Line::Ansi { stripped, .. } => stripped,
        }
    }

    /// Returns the raw version of the line (with ANSI codes intact).
    pub fn raw(&self) -> &str {
        match self {
            Line::Clean(s) => s,
            Line::Ansi { raw, .. } => raw,
        }
    }

    /// Length of the plain text in bytes.
    pub fn len(&self) -> usize {
        self.display().len()
    }

    /// Returns true if the display/stripped version is empty.
    pub fn is_empty(&self) -> bool {
        self.display().is_empty()
    }

    /// Creates a new Line from raw text, automatically detecting ANSI codes.
    pub fn new(raw: String) -> Self {
        if !raw.chars().any(char::is_control) {
            return Self::Clean(raw);
        }
        let mut plain = PlainText::default();
        Parser::new().advance(&mut plain, raw.as_bytes());
        if plain.text == raw {
            return Self::Clean(raw);
        }
        Self::Ansi {
            raw,
            stripped: plain.text,
        }
    }

    pub(super) fn from_scrollback(raw: String, tab_width: usize) -> Self {
        if !raw.contains('\t') {
            return Self::new(raw);
        }
        let mut expanded = String::with_capacity(raw.len());
        let mut parser = Parser::new();
        let mut since_tab = PlainText::default();
        for part in raw.split_inclusive('\t') {
            if let Some(before_tab) = part.strip_suffix('\t') {
                parser.advance(&mut since_tab, part.as_bytes());
                expanded.push_str(before_tab);
                if since_tab.tab {
                    // Each preceding tab ended at a tab stop. Styles occupy no cells.
                    if tab_width > 0 {
                        let spaces = tab_width - (since_tab.text.width() % tab_width);
                        expanded.extend(std::iter::repeat_n(' ', spaces));
                    }
                    since_tab.text.clear();
                    since_tab.tab = false;
                } else {
                    // Part of a control string, rather than a displayed tab.
                    expanded.push('\t');
                }
            } else {
                expanded.push_str(part);
            }
        }
        Self::new(expanded)
    }

    /// Leftmost visible byte when a line is wider than the terminal.
    pub(super) fn screen_start(&self, column: usize, width: usize) -> usize {
        let text = self.display();
        if text.width() <= width {
            return 0;
        }
        let cursor = get_utf_index(text, column);
        let (start, glyph_width) = text
            .grapheme_indices(true)
            .find(|(i, g)| cursor < i + g.len())
            .map_or((text.len(), 1), |(i, g)| (i, g.width().max(1)));
        start - suffix_by_width(&text[..start], width.saturating_sub(glyph_width)).len()
    }

    pub(super) fn for_screen(&self, start: usize, width: usize) -> Cow<'_, str> {
        let visible = prefix_by_width(&self.display()[start..], width);
        match self {
            Self::Clean(_) => Cow::Borrowed(visible),
            Self::Ansi { raw, .. } => {
                let mut row = StyledRow {
                    text: String::with_capacity(visible.len()),
                    skip: start,
                    remaining: visible.len(),
                };
                Parser::new().advance(&mut row, raw.as_bytes());
                Cow::Owned(row.text)
            }
        }
    }
}

#[derive(Default)]
struct PlainText {
    text: String,
    tab: bool,
}

impl Perform for PlainText {
    fn print(&mut self, c: char) {
        if !c.is_control() {
            self.text.push(c);
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\t' => self.tab = true,
            b'\n' => self.text.push('\n'),
            _ => {}
        }
    }
}

/// Scrollback may contain terminal commands. Only text and SGR attributes belong in a repaint.
struct StyledRow {
    text: String,
    skip: usize,
    remaining: usize,
}

impl Perform for StyledRow {
    fn print(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        if self.skip > 0 {
            self.skip -= c.len_utf8();
        } else if c.len_utf8() <= self.remaining {
            self.text.push(c);
            self.remaining -= c.len_utf8();
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if action != 'm' || !intermediates.is_empty() || ignore {
            return;
        }
        self.text.push_str("\x1b[");
        for (i, param) in params.iter().enumerate() {
            if i != 0 {
                self.text.push(';');
            }
            for (j, value) in param.iter().enumerate() {
                if j != 0 {
                    self.text.push(':');
                }
                write!(self.text, "{value}").expect("writing to a String");
            }
        }
        self.text.push('m');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repaint_preserves_styles_and_clips_whole_graphemes() {
        let line = Line::new("\x1b(B\x1b[1;34mA👩‍💻Z\x1b[0m".into());
        assert_eq!(line.display(), "A👩‍💻Z");
        assert_eq!(line.for_screen(0, 2), "\x1b[1;34mA\x1b[0m");
        assert_eq!(line.for_screen(0, 3), "\x1b[1;34mA👩‍💻\x1b[0m");
        let start = line.screen_start(4, 3);
        assert_eq!(line.for_screen(start, 3), "\x1b[1;34m👩‍💻Z\x1b[0m");
    }

    #[test]
    fn aligned_tabs_advance_a_full_stop_and_zero_width_removes_them() {
        assert_eq!(
            Line::from_scrollback("abcd\t\te".into(), 4).display(),
            "abcd        e"
        );
        assert_eq!(Line::from_scrollback("a\tb".into(), 0).display(), "ab");
    }
}
