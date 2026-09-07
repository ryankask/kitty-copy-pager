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
