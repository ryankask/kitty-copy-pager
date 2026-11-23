use super::Pager;
use super::search::{Search, SearchDirection, SearchState};
use crate::selection::Selection;
use crate::utils::{get_utf_index, prefix_by_width, suffix_by_width};
use crossterm::{
    QueueableCommand,
    cursor::MoveTo,
    style::{Attribute, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate},
};
use std::io::{self, Write, stdout};
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

/// Finish composing the frame before kitty starts its synchronized update.
fn paint_frame(paint: impl FnOnce(&mut Vec<u8>) -> io::Result<()>) -> io::Result<()> {
    let mut frame = Vec::new();
    frame.queue(BeginSynchronizedUpdate)?;
    paint(&mut frame)?;
    frame.queue(EndSynchronizedUpdate)?;
    let mut out = stdout().lock();
    out.write_all(&frame)?;
    out.flush()
}

impl Pager {
    fn row_start_byte(&self, y: usize) -> usize {
        if y == self.logical_y {
            self.line_start_byte
        } else {
            0
        }
    }

    pub fn draw_status_line(&self) -> io::Result<()> {
        paint_frame(|out| self.write_status_line(out))
    }

    fn write_status_line(&self, out: &mut impl Write) -> io::Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        let width = self.term_width;
        let row = (self.term_height - 1) as u16;
        let line = self.current_line();
        let cursor_byte = get_utf_index(line, self.cursor_x);
        let mut cursor = (
            line[self.line_start_byte..cursor_byte]
                .width()
                .min(width - 1) as u16,
            (self.logical_y - self.viewport_start) as u16,
        );
        if self.term_height == 1
            && !self
                .search
                .as_ref()
                .is_some_and(|s| s.state == SearchState::Typing || s.error.is_some())
        {
            out.queue(MoveTo(cursor.0, cursor.1))?;
            return Ok(());
        }

        let status = format!("Ln {}, Col {}", self.logical_y, self.cursor_x);
        out.queue(MoveTo(0, row))?;
        out.queue(SetAttribute(Attribute::Reset))?;
        out.queue(SetBackgroundColor(self.settings.status_line_bg_color))?;
        out.queue(SetForegroundColor(self.settings.status_line_fg_color))?;
        out.queue(Clear(ClearType::CurrentLine))?;

        if let Some(search) = &self.search {
            if search.state == SearchState::Submitted
                && let Some(error) = search.error
            {
                out.queue(SetForegroundColor(self.settings.search_error_fg_color))?;
                out.queue(Print(prefix_by_width(error, width)))?;
            } else {
                let prefix = match search.direction {
                    SearchDirection::Forward => '/',
                    SearchDirection::Backward => '?',
                };
                let counter = search
                    .current_result_index
                    .map(|i| format!("[{}/{}] ", i + 1, search.results.len()))
                    .unwrap_or_default();
                let typing = search.state == SearchState::Typing;
                let query_width = search.query.width();
                let show_status = 1 + query_width + 1 + counter.len() + status.len() <= width;
                let room = width.saturating_sub(1 + usize::from(typing));
                let clipped = query_width > room && room > 0;
                let room = room - usize::from(clipped);
                let query = if typing {
                    suffix_by_width(&search.query, room)
                } else {
                    prefix_by_width(&search.query, room)
                };
                out.queue(Print(prefix))?;
                if clipped && typing {
                    out.queue(Print('…'))?;
                }
                out.queue(Print(query))?;
                if clipped && !typing {
                    out.queue(Print('…'))?;
                }
                if show_status {
                    out.queue(MoveTo((width - status.len() - counter.len()) as u16, row))?;
                    out.queue(Print(counter))?;
                    out.queue(Print(&status))?;
                }
                if typing {
                    cursor = (
                        (1 + usize::from(clipped) + query.width()).min(width - 1) as u16,
                        row,
                    );
                }
            }
        } else {
            out.queue(MoveTo(width.saturating_sub(status.len()) as u16, row))?;
            out.queue(Print(prefix_by_width(&status, width)))?;
        }
        out.queue(MoveTo(cursor.0, cursor.1))?;
        Ok(())
    }

    pub fn draw(&self) -> io::Result<()> {
        self.draw_rows(self.visible_lines())
    }

    pub(super) fn draw_rows(&self, rows: Range<usize>) -> io::Result<()> {
        if self.is_empty() {
            return Ok(());
        }
        let visible = self.visible_lines();
        let rows = rows.start.max(visible.start)..rows.end.min(visible.end);
        if rows.is_empty() {
            return self.draw_status_line();
        }
        paint_frame(|out| {
            out.queue(SetAttribute(Attribute::Reset))?;
            for y in rows.clone() {
                let line = &self.lines[y].text.display()[self.row_start_byte(y)..];
                let used = prefix_by_width(line, self.term_width).width();
                if used < self.term_width {
                    // Remove leftover text and highlighted newlines in the default background.
                    out.queue(MoveTo(used as u16, (y - visible.start) as u16))?;
                    out.queue(Clear(ClearType::UntilNewLine))?;
                }
            }
            // A color or attribute can start above the first row being repainted.
            for row in &self.lines[visible.start..rows.start] {
                out.queue(Print(row.text.style_changes()))?;
            }
            for y in rows.clone() {
                out.queue(MoveTo(0, (y - visible.start) as u16))?;
                out.queue(Print(
                    self.lines[y]
                        .text
                        .for_screen(self.row_start_byte(y), self.term_width),
                ))?;
            }

            if let Some(search) = &self.search
                && !search.results.is_empty()
            {
                self.draw_matches(search, rows.clone(), out)?;
            }
            if let Some(selection) = &self.selection {
                self.draw_selection(selection, rows, out)?;
            }
            self.write_status_line(out)
        })
    }

    fn draw_matches(
        &self,
        search: &Search,
        rows: Range<usize>,
        out: &mut impl Write,
    ) -> io::Result<()> {
        out.queue(SetAttribute(Attribute::Reset))?;
        out.queue(SetForegroundColor(self.settings.search_highlight_fg_color))?;
        out.queue(SetBackgroundColor(self.settings.search_highlight_bg_color))?;
        for y in rows {
            let line = self.lines[y].text.display();
            let left = self.row_start_byte(y);
            let text = prefix_by_width(&line[left..], self.term_width);
            let first_col = line[..left].chars().count();
            let end_col = first_col + text.chars().count();
            let first = search
                .results
                .partition_point(|r| (r.line_index, r.columns.end) <= (y, first_col));
            for hit in search.results[first..]
                .iter()
                .take_while(|r| (r.line_index, r.columns.start) < (y, end_col))
            {
                if hit.columns.is_empty() {
                    continue;
                }
                let start = get_utf_index(text, hit.columns.start.saturating_sub(first_col));
                let end = get_utf_index(text, hit.columns.end.min(end_col) - first_col);
                out.queue(MoveTo(
                    text[..start].width() as u16,
                    (y - self.viewport_start) as u16,
                ))?;
                out.queue(Print(&text[start..end]))?;
            }
        }
        Ok(())
    }

    fn draw_selection(
        &self,
        selection: &Selection,
        rows: Range<usize>,
        out: &mut impl Write,
    ) -> io::Result<()> {
        let (start, end) = (&selection.start, &selection.end);
        if start == end || end.y < rows.start || start.y >= rows.end {
            return Ok(());
        }
        out.queue(SetAttribute(Attribute::Reset))?;
        out.queue(SetForegroundColor(self.settings.selection_fg_color))?;
        out.queue(SetBackgroundColor(self.settings.selection_bg_color))?;
        for y in start.y.max(rows.start)..=end.y.min(rows.end - 1) {
            let line = self.lines[y].text.display();
            let left = self.row_start_byte(y);
            let start_byte = if y == start.y {
                get_utf_index(line, start.x).max(left)
            } else {
                left
            };
            let end_byte = if y == end.y {
                get_utf_index(line, end.x)
            } else {
                line.len()
            };
            if end_byte < start_byte {
                continue;
            }
            let x = line[left..start_byte].width();
            if x >= self.term_width {
                continue;
            }
            let available = self.term_width - x;
            let text = prefix_by_width(&line[start_byte..end_byte], available);
            out.queue(MoveTo(x as u16, (y - self.viewport_start) as u16))?;
            out.queue(Print(text))?;
            if y < end.y
                && !self.lines[y + 1].continues_previous
                && text.len() == end_byte - start_byte
                && text.width() < available
            {
                // A selected newline occupies the first cell after the text.
                out.queue(Print(' '))?;
            }
        }
        Ok(())
    }
}
