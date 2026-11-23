use super::Pager;
use crate::utils::{WordCharExt, first_non_whitespace_idx_on, get_utf_index};
use std::io;

/// Count the intervening whitespace and the following run of word or punctuation characters.
/// Passing the characters in reverse gives the distance to the previous word start.
fn word_distance(chars: impl Iterator<Item = char>) -> Option<usize> {
    let mut chars = chars.enumerate();
    let (whitespace, first) = chars.find(|(_, c)| !c.is_whitespace())?;
    let rest = chars
        .take_while(|(_, c)| {
            !c.is_whitespace() && c.is_word_punctuation() == first.is_word_punctuation()
        })
        .count();
    Some(whitespace + 1 + rest)
}

impl Pager {
    pub(crate) fn back_to_indentation(&mut self) -> io::Result<()> {
        let column = first_non_whitespace_idx_on(self.current_line())
            .unwrap_or_else(|| self.current_line_len());
        self.move_to(column, self.logical_y)
    }

    pub(crate) fn previous_word(&mut self) -> io::Result<()> {
        for y in (0..=self.logical_y).rev() {
            let line = self.lines[y].text.display();
            let (prefix, column) = if y == self.logical_y {
                (&line[..get_utf_index(line, self.cursor_x)], self.cursor_x)
            } else {
                (line, line.chars().count())
            };
            if let Some(distance) = word_distance(prefix.chars().rev()) {
                return self.move_to(column - distance, y);
            }
        }
        self.move_to(0, 0)
    }

    pub(crate) fn next_word_end(&mut self) -> io::Result<()> {
        for y in self.logical_y..self.lines.len() {
            let line = self.lines[y].text.display();
            let column = if y == self.logical_y {
                self.cursor_x
            } else {
                0
            };
            let suffix = &line[get_utf_index(line, column)..];
            if let Some(distance) = word_distance(suffix.chars()) {
                return self.move_to(column + distance, y);
            }
        }
        self.move_to(usize::MAX, self.lines.len() - 1)
    }
}
