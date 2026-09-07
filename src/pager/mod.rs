use crate::pager::search::{Search, SearchDirection, SearchState};
use crate::selection::*;
use crate::settings::Settings;
use crossterm::event::KeyEvent;
use std::io::{self, stdin};
use strip_ansi_escapes::strip;

mod copy;
mod keybindings;
mod modes;
mod motions;
mod movement;
mod rendering;
mod search;

/// Represents a line of text, optimized for memory when no ANSI codes are present.
/// When a line contains ANSI escape codes, we store both the raw version (for copying)
/// and the stripped version (for display and logic).
/// When a line has no ANSI codes, we only store one String.
#[derive(Clone)]
pub enum Line {
    /// Line without ANSI escape codes - display and raw are the same
    Clean(String),
    /// Line with ANSI escape codes - need to store both
    Ansi { raw: String, stripped: String },
}

impl Line {
    /// Returns the display/stripped version of the line (without ANSI codes).
    /// This is used for rendering, cursor positioning, and search.
    pub fn display(&self) -> &str {
        match self {
            Line::Clean(s) => s,
            Line::Ansi { stripped, .. } => stripped,
        }
    }

    /// Returns the raw version of the line (with ANSI codes intact).
    /// This is used when copying text to preserve formatting.
    pub fn raw(&self) -> &str {
        match self {
            Line::Clean(s) => s,
            Line::Ansi { raw, .. } => raw,
        }
    }

    /// Returns the length of the display/stripped version.
    pub fn len(&self) -> usize {
        self.display().len()
    }

    /// Returns true if the display/stripped version is empty.
    pub fn is_empty(&self) -> bool {
        self.display().is_empty()
    }

    /// Creates a new Line from raw text, automatically detecting ANSI codes.
    pub fn new(raw: String) -> Self {
        let stripped_bytes = strip(raw.as_bytes());
        if stripped_bytes.len() == raw.len() && stripped_bytes == raw.as_bytes() {
            // No ANSI codes found, no stripping occurred
            Line::Clean(raw)
        } else {
            // ANSI codes were stripped, store both versions
            let stripped = String::from_utf8_lossy(&stripped_bytes).into_owned();
            Line::Ansi { raw, stripped }
        }
    }
}

pub struct Pager {
    pub(crate) lines: Vec<Line>,
    pub(crate) cursor_x: usize, // This is both the physical and logical position
    pub(crate) wish_cursor_x: usize,
    pub(crate) logical_y: usize,
    pub(crate) term_height: usize,
    pub(crate) term_width: usize,
    pub(crate) viewport_start: usize,
    pub(crate) viewport_end: usize,
    pub(crate) selection: Option<Selection>, // We'll assume that start is always before end
    pub(crate) search: Option<Search>,
    pub(crate) settings: Settings,
}

impl Pager {
    pub fn new(settings: Settings) -> io::Result<Self> {
        let mut lines = Vec::<Line>::new();
        let tab_replacement = String::from(" ").repeat(settings.tab_width);
        for line in stdin().lines() {
            let mut line = line?;
            if line.contains('\t') {
                line = line.replace('\t', &tab_replacement);
            }
            // Create Line - will automatically detect ANSI and choose appropriate variant
            lines.push(Line::new(line));
        }
        let (term_width, term_height) = crossterm::terminal::size()?;

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

            term_height: term_height as usize,
            term_width: term_width as usize,

            viewport_start: lines
                .len()
                .saturating_sub((term_height as usize).saturating_sub(1)),
            viewport_end: lines.len().saturating_sub(1),

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
        if let Some(search) = &self.search
            && search.state == SearchState::Typing
        {
            if let Some(direction) = self.search_mode(event)? {
                self.search();
                if let Some(search) = &self.search
                    && search.error.is_some()
                {
                    self.draw()?;
                } else {
                    self.move_to_closest_match(direction == SearchDirection::Forward)?;
                }
            }
            Ok(false)
        } else {
            self.normal_mode(event)
        }
    }

    /// Gets the current line (display/stripped version)
    fn current_line(&self) -> &str {
        self.lines[self.logical_y].display()
    }

    /// Gets the current line length (display/stripped version)
    /// Warning: gets the utf-8 length
    fn current_line_len(&self) -> usize {
        self.current_line().chars().count()
    }
}
