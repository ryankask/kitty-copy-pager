use super::{Line, Pager};

use crate::selection::*;
use crate::utils::get_utf_index;
use crossterm::{clipboard::CopyToClipboard, execute};
use std::io::{self, Write, stdout};

fn selected_text(lines: &[Line], selection: &Selection) -> String {
    let mut text = String::new();
    let end_y = selection.end.y.min(lines.len().saturating_sub(1));
    let last_i = end_y.saturating_sub(selection.start.y);

    for (i, line) in lines[selection.start.y..=end_y].iter().enumerate() {
        let line = line.display();
        if i == 0 && i == last_i {
            let start = get_utf_index(line, selection.start.x);
            let end = get_utf_index(line, selection.end.x);
            text.push_str(&line[start..end]);
        } else if i == 0 {
            let start = get_utf_index(line, selection.start.x);
            text.push_str(&line[start..]);
            text.push('\n');
        } else if i == last_i {
            let end = get_utf_index(line, selection.end.x);
            text.push_str(&line[..end]);
        } else {
            text.push_str(line);
            text.push('\n');
        }
    }

    text
}

impl Pager {
    pub(crate) fn copy_end_of_line(&mut self) -> io::Result<()> {
        let end_x = self.current_line_len();
        self.selection = Some(Selection::with_coords(
            self.cursor_x,
            self.logical_y,
            end_x,
            self.logical_y,
        ));
        self.copy_selection()
    }

    pub(crate) fn expand_selection(&mut self) {
        let y = self.logical_y;
        let Some(sel) = &mut self.selection else {
            return;
        };
        match sel.sel_end {
            SelectedEnd::Start => {
                if y > sel.end.y || (y == sel.end.y && self.cursor_x > sel.end.x) {
                    sel.swap_ends_to(self.cursor_x, y);
                    sel.sel_end = SelectedEnd::End;
                } else {
                    sel.start = Vec2::new(self.cursor_x, y);
                }
            }
            SelectedEnd::End => {
                if y < sel.start.y || (y == sel.start.y && self.cursor_x < sel.start.x) {
                    sel.swap_ends_to(self.cursor_x, y);
                    sel.sel_end = SelectedEnd::Start;
                } else {
                    sel.end = Vec2::new(self.cursor_x, y);
                }
            }
        }
    }

    pub(crate) fn copy_selection(&self) -> io::Result<()> {
        let Some(sel) = &self.selection else {
            return Ok(());
        };
        if sel.start == sel.end {
            return Ok(());
        }

        let text = selected_text(&self.lines, sel);
        let mut out = stdout();
        execute!(out, CopyToClipboard::to_clipboard_from(&text))?;
        out.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(input: &[&str]) -> Vec<Line> {
        input
            .iter()
            .map(|line| Line::new((*line).to_string()))
            .collect()
    }

    #[test]
    fn selection_end_is_exclusive() {
        let lines = lines(&["alpha"]);
        let selection = Selection::with_coords(0, 0, 4, 0);

        assert_eq!(selected_text(&lines, &selection), "alph");
    }

    #[test]
    fn selection_can_end_after_the_last_character() {
        let lines = lines(&["alpha"]);
        let selection = Selection::with_coords(0, 0, 5, 0);

        assert_eq!(selected_text(&lines, &selection), "alpha");
    }

    #[test]
    fn multiline_selection_has_no_unselected_trailing_newline() {
        let lines = lines(&["one", "two"]);
        let selection = Selection::with_coords(0, 0, 3, 1);

        assert_eq!(selected_text(&lines, &selection), "one\ntwo");
    }

    #[test]
    fn unicode_selection_uses_character_positions() {
        let lines = lines(&["aé🙂"]);
        let selection = Selection::with_coords(0, 0, 3, 0);

        assert_eq!(selected_text(&lines, &selection), "aé🙂");
    }
}
