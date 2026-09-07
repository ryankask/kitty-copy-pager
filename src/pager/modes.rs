use super::keybindings::{Action, action_for};
use super::{Pager, SearchDirection, SearchState};

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
            Action::PreviousWord => self.movement_b(false, false)?,
            Action::NextWordEnd => self.movement_e(false, false)?,
            Action::BeginningOfLine => self.move_horizontally_to(0)?,
            Action::EndOfLine => self.movement_dollar()?,
            Action::BackToIndentation => self.movement_underscore()?,
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
                self.selection = Some(Selection::with_coords(
                    self.cursor_x,
                    self.logical_y,
                    self.cursor_x,
                    self.logical_y,
                ));
                self.draw()?;
            }
            Action::ClearSelection => {
                self.selection = None;
                if let Some(search) = &mut self.search {
                    search.state = SearchState::Hidden;
                }
                self.draw()?;
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
