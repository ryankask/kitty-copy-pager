use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) fn prefix_by_width(text: &str, mut cells: usize) -> &str {
    for (i, grapheme) in text.grapheme_indices(true) {
        let width = grapheme.width();
        if width > cells {
            return &text[..i];
        }
        cells -= width;
    }
    text
}

pub(crate) fn suffix_by_width(text: &str, mut cells: usize) -> &str {
    for (i, grapheme) in text.grapheme_indices(true).rev() {
        let width = grapheme.width();
        if width > cells {
            return &text[i + grapheme.len()..];
        }
        cells -= width;
    }
    text
}

pub fn get_utf_index(line: &str, idx: usize) -> usize {
    line.char_indices()
        .nth(idx)
        .map(|(i, _)| i)
        .unwrap_or(line.len())
}

pub trait WordCharExt {
    fn is_word_punctuation(&self) -> bool;
}

impl WordCharExt for char {
    fn is_word_punctuation(&self) -> bool {
        self.is_ascii_punctuation()
    }
}

/// Logical index in utf-8 indices
pub fn first_non_whitespace_idx_on(line: &str) -> Option<usize> {
    for (i, c) in line.chars().enumerate() {
        if !c.is_whitespace() {
            return Some(i);
        }
    }
    None
}
