use crate::pager::search::{Search, SearchDirection, SearchState};
use crate::selection::*;
use crate::settings::Settings;
use crossterm::event::{KeyEvent, KeyEventKind};
pub use line::Line;
use std::io::{self, stdin};

mod copy;
mod keybindings;
mod line;
mod modes;
mod motions;
mod movement;
mod rendering;
mod search;

pub struct Pager {
    pub(crate) lines: Vec<Line>,
    pub(crate) cursor_x: usize, // Character offset, including the position after the last character.
    pub(crate) wish_cursor_x: usize,
    pub(crate) logical_y: usize,
    pub(crate) term_height: usize,
    pub(crate) term_width: usize,
    pub(crate) viewport_start: usize,
    pub(crate) line_start_byte: usize,
    pub(crate) selection: Option<Selection>,
    pub(crate) search: Option<Search>,
    pub(crate) settings: Settings,
}

impl Pager {
    pub fn new(settings: Settings) -> io::Result<Self> {
        let mut lines = Vec::<Line>::new();
        for line in stdin().lines() {
            lines.push(Line::from_scrollback(line?, settings.tab_width));
        }
        let (term_width, term_height) = if lines.is_empty() {
            (1, 1)
        } else {
            crossterm::terminal::size()?
        };
        let term_width = usize::from(term_width).max(1);
        let term_height = usize::from(term_height).max(1);

        // The pager may contain empty lines at the end
        let mut last_non_empty_line_idx = lines.len().saturating_sub(1);
        while last_non_empty_line_idx > 0 && lines[last_non_empty_line_idx].display().is_empty() {
            last_non_empty_line_idx -= 1;
        }
        lines.truncate(last_non_empty_line_idx + 1);

        let cursor_x = lines
            .last()
            .map(|line| {
                let line_len = line.display().chars().count();
                line_len
                    .saturating_sub(1)
                    .saturating_add(settings.prompt_cursor_offset)
                    .min(line_len)
            })
            .unwrap_or(0);

        Ok(Self {
            cursor_x,
            logical_y: lines.len().saturating_sub(1),

            wish_cursor_x: cursor_x,

            term_height,
            term_width,

            viewport_start: lines
                .len()
                .saturating_sub(term_height.saturating_sub(1).max(1)),
            line_start_byte: lines
                .last()
                .map_or(0, |line| line.screen_start(cursor_x, term_width)),

            lines,

            selection: None,
            search: None,
            settings,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn handle_key_event(&mut self, event: KeyEvent) -> io::Result<bool> {
        if self.is_empty() {
            return Ok(true);
        }
        if event.kind == KeyEventKind::Release {
            return Ok(false);
        }
        if self
            .search
            .as_ref()
            .is_some_and(|s| s.state == SearchState::Typing)
        {
            self.search_mode(event)?;
            Ok(false)
        } else {
            self.normal_mode(event)
        }
    }

    /// Gets the current line (display/stripped version)
    fn current_line(&self) -> &str {
        self.lines[self.logical_y].display()
    }

    /// Length in characters, excluding terminal formatting.
    fn current_line_len(&self) -> usize {
        self.current_line().chars().count()
    }
}
