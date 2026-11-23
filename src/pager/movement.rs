use super::Pager;
use std::io;
use std::ops::Range;

impl Pager {
    pub(crate) fn visible_lines(&self) -> Range<usize> {
        let end = (self.viewport_start + self.visible_rows()).min(self.lines.len());
        self.viewport_start..end
    }

    fn visible_rows(&self) -> usize {
        self.term_height.saturating_sub(1).max(1)
    }

    /// Place point, extend any active region, and keep point inside the viewport.
    /// Return the scrollback rows whose contents or highlighting changed.
    pub(crate) fn set_cursor(&mut self, x: usize, y: usize) -> Range<usize> {
        let previous_line = self.logical_y;
        let previous_byte = self.line_start_byte;
        self.logical_y = y.min(self.lines.len() - 1);
        self.cursor_x = x.min(self.current_line_len());
        self.wish_cursor_x = self.cursor_x;
        self.line_start_byte =
            self.lines[self.logical_y].screen_start(self.cursor_x, self.term_width);
        self.expand_selection();

        let rows = self.visible_rows();
        let margin = self.settings.scrolloff.min((rows - 1) / 2);
        let first = (self.logical_y + margin + 1).saturating_sub(rows);
        let last = self.logical_y.saturating_sub(margin);
        let start = self
            .viewport_start
            .clamp(first, last)
            .min(self.lines.len().saturating_sub(rows));
        let scrolled = start != self.viewport_start;
        self.viewport_start = start;
        if scrolled {
            self.visible_lines()
        } else if self.selection.is_some()
            || previous_byte != self.line_start_byte
            || (previous_line != self.logical_y && previous_byte != 0)
        {
            previous_line.min(self.logical_y)..previous_line.max(self.logical_y) + 1
        } else {
            0..0
        }
    }

    pub(crate) fn move_to(&mut self, x: usize, y: usize) -> io::Result<()> {
        let rows = self.set_cursor(x, y);
        self.draw_rows(rows)
    }

    pub(crate) fn move_vertically_by(&mut self, amount: isize) -> io::Result<()> {
        let column = self.wish_cursor_x;
        let result = self.move_to(column, self.logical_y.saturating_add_signed(amount));
        self.wish_cursor_x = column;
        result
    }

    pub(crate) fn move_char_backward(&mut self) -> io::Result<()> {
        if self.cursor_x > 0 {
            self.move_to(self.cursor_x - 1, self.logical_y)
        } else if self.logical_y > 0 {
            self.move_to(usize::MAX, self.logical_y - 1)
        } else {
            Ok(())
        }
    }

    pub(crate) fn move_char_forward(&mut self) -> io::Result<()> {
        if self.cursor_x < self.current_line_len() {
            self.move_to(self.cursor_x + 1, self.logical_y)
        } else if self.logical_y + 1 < self.lines.len() {
            self.move_to(0, self.logical_y + 1)
        } else {
            Ok(())
        }
    }

    pub(crate) fn recenter(&mut self) -> io::Result<()> {
        let rows = self.visible_rows();
        self.viewport_start = self
            .logical_y
            .saturating_sub(rows / 2)
            .min(self.lines.len().saturating_sub(rows));
        self.draw()
    }
}
