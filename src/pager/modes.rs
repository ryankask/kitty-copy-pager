use super::keybindings::{Action, action_for};
use super::{Pager, SearchDirection};

use crate::selection::Selection;
use crossterm::event::KeyEvent;
use std::io;

impl Pager {
    pub(crate) fn normal_mode(&mut self, event: KeyEvent) -> io::Result<bool> {
        let Some(action) = action_for(event) else {
            return Ok(false);
        };

        match action {
            Action::BackwardChar => self.move_char_backward()?,
            Action::ForwardChar => self.move_char_forward()?,
            Action::PreviousLine => self.move_vertically_by(-1)?,
            Action::NextLine => self.move_vertically_by(1)?,
            Action::PreviousWord => self.previous_word()?,
            Action::NextWordEnd => self.next_word_end()?,
            Action::BeginningOfLine => self.move_to(0, self.logical_y)?,
            Action::EndOfLine => self.move_to(usize::MAX, self.logical_y)?,
            Action::BackToIndentation => self.back_to_indentation()?,
            Action::BeginningOfBuffer => self.move_to(0, 0)?,
            Action::EndOfBuffer => {
                self.move_to(usize::MAX, self.lines.len().saturating_sub(1))?;
            }
            Action::PageUp => {
                let page = self.page_size();
                self.move_vertically_by(-page)?;
            }
            Action::PageDown => {
                let page = self.page_size();
                self.move_vertically_by(page)?;
            }
            Action::Recenter => self.recenter()?,
            Action::BeginSelection => {
                let previous = self.selection.replace(Selection::with_coords(
                    self.cursor_x,
                    self.logical_y,
                    self.cursor_x,
                    self.logical_y,
                ));
                if let Some(old) = previous {
                    self.draw_rows(old.start.y..old.end.y + 1)?;
                }
            }
            Action::ClearSelection => {
                let previous = self.selection.take();
                if self.search.take().is_some() {
                    self.draw()?;
                } else if let Some(old) = previous {
                    self.draw_rows(old.start.y..old.end.y + 1)?;
                }
            }
            Action::CopySelection => {
                if self.selection.is_some() {
                    self.copy_selection()?;
                    return Ok(true);
                }
            }
            Action::CopyEndOfLine => {
                self.copy_end_of_line()?;
                return Ok(true);
            }
            Action::SearchForward => {
                self.begin_or_repeat_search(SearchDirection::Forward)?;
            }
            Action::SearchBackward => {
                self.begin_or_repeat_search(SearchDirection::Backward)?;
            }
            Action::Quit => return Ok(true),
        }
        Ok(false)
    }

    fn page_size(&self) -> isize {
        self.term_height.saturating_sub(2).max(1) as isize
    }
}
