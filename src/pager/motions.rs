use super::Pager;

use crate::utils::{WordCharExt, get_utf_index};
use std::io::{self};

impl Pager {
    pub(crate) fn movement_underscore(&mut self) -> io::Result<()> {
        let mut jmp = 0;
        for (i, c) in self.current_line().chars().enumerate() {
            if !c.is_whitespace() {
                jmp = i;
                break;
            }
        }
        self.move_horizontally_to(jmp)
    }

    #[inline(always)]
    fn jump_condition(c: char, peek: char) -> bool {
        // Case 1: 'a,'
        // Case 2: ',a' (but not ', ')
        (c.is_word_punctuation() && !peek.is_word_punctuation())
            || (!c.is_word_punctuation() && !c.is_whitespace() && peek.is_word_punctuation())
    }

    fn find_b_jump(&self, whitespace_only: bool, _already_wrapped: bool) -> Option<usize> {
        let line = self.current_line();
        let search_end = self.cursor_x;
        let end = get_utf_index(line, search_end);
        for (i, c) in line[..end].chars().rev().enumerate() {
            let target_idx = search_end.saturating_sub(1).saturating_sub(i);
            let prev = if target_idx == 0 {
                'a'
            } else {
                line.chars().nth(target_idx - 1).unwrap_or('a')
            };
            if (!c.is_whitespace()
                && (prev.is_whitespace() || (target_idx == 0 && self.cursor_x != 0)))
                || (!whitespace_only && Self::jump_condition(c, prev))
            {
                return Some(self.cursor_x.saturating_sub(target_idx));
            }
        }
        None
    }

    pub(crate) fn movement_b(
        &mut self,
        whitespace_only: bool,
        already_wrapped: bool,
    ) -> io::Result<()> {
        match self.find_b_jump(whitespace_only, already_wrapped) {
            Some(jump) => {
                self.move_horizontally_by(-(jump as isize))?;
            }
            None => {
                if !already_wrapped {
                    self.wrap_to_previous(whitespace_only, true, Pager::movement_b)?;
                }
            }
        }
        Ok(())
    }

    fn find_e_jump(&self, whitespace_only: bool, _already_wrapped: bool) -> Option<usize> {
        let line = self.current_line();
        let search_start = self.cursor_x;
        let start = get_utf_index(line, search_start);
        let line_end = &line[start..];
        let last_idx = line_end.chars().count().saturating_sub(1);

        for (i, c) in line_end.chars().enumerate() {
            let peek = line_end.chars().nth(i + 1).unwrap_or('a');
            if i == last_idx
                || (!c.is_whitespace() && peek.is_whitespace())
                || (!whitespace_only && Self::jump_condition(peek, c))
            {
                let target_idx = search_start.saturating_add(i + 1);
                return Some(target_idx.saturating_sub(self.cursor_x));
            }
        }
        None
    }

    pub(crate) fn movement_e(
        &mut self,
        whitespace_only: bool,
        already_wrapped: bool,
    ) -> io::Result<()> {
        match self.find_e_jump(whitespace_only, already_wrapped) {
            Some(jump) => {
                self.move_horizontally_by(jump as isize)?;
            }
            None => {
                if !already_wrapped {
                    self.wrap_to_next(whitespace_only, true, Pager::movement_e)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn movement_dollar(&mut self) -> io::Result<()> {
        self.move_horizontally_to(self.current_line_len())
    }
}
