use super::{Pager, ScrollbackRow};
use crate::selection::Vec2;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regex::RegexBuilder;
use std::io;
use std::ops::Range;

pub(crate) struct SearchResult {
    pub line_index: usize,
    /// Half-open character range, in the same units as the cursor and selection.
    pub columns: Range<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchDirection {
    Forward,
    Backward,
}

#[derive(PartialEq, Eq)]
pub(crate) enum SearchState {
    Typing,
    Submitted,
}

pub(crate) struct Search {
    pub query: String,
    pub direction: SearchDirection,
    pub state: SearchState,
    pub results: Vec<SearchResult>,
    pub current_result_index: Option<usize>,
    pub error: Option<&'static str>,
    origin: Vec2<usize>,
    origin_viewport: usize,
    origin_wish_x: usize,
    /// An explicit repeat moves the starting point for further edits to that match.
    repeat_anchor: Option<Vec2<usize>>,
}

impl Search {
    fn new(pager: &Pager, direction: SearchDirection) -> Self {
        let origin = Vec2::new(pager.cursor_x, pager.logical_y);
        Self {
            query: String::new(),
            direction,
            state: SearchState::Typing,
            results: Vec::new(),
            current_result_index: None,
            error: None,
            repeat_anchor: None,
            origin,
            origin_viewport: pager.viewport_start,
            origin_wish_x: pager.wish_cursor_x,
        }
    }

    fn update_matches(&mut self, lines: &[ScrollbackRow], smart_case: bool) {
        self.results.clear();
        self.current_result_index = None;
        if self.query.is_empty() {
            self.error = Some("Error: empty search query");
            return;
        }

        let regex = RegexBuilder::new(&self.query)
            .case_insensitive(smart_case && !self.query.chars().any(char::is_uppercase))
            .build();
        let Ok(regex) = regex else {
            self.error = Some("Error: invalid regex pattern");
            return;
        };

        for (line_index, row) in lines.iter().enumerate() {
            let text = row.text.display();
            let mut byte = 0;
            let mut column = 0;
            for mat in regex.find_iter(text) {
                let start = column + text[byte..mat.start()].chars().count();
                column = start + mat.as_str().chars().count();
                byte = mat.end();
                self.results.push(SearchResult {
                    line_index,
                    columns: start..column,
                });
            }
        }
        self.error = self
            .results
            .is_empty()
            .then_some("Error: could not find any occurrences");
        let include_cursor =
            self.direction == SearchDirection::Forward || self.repeat_anchor.is_some();
        self.current_result_index = self.index_from(self.anchor(), include_cursor);
    }

    fn anchor(&self) -> &Vec2<usize> {
        self.repeat_anchor.as_ref().unwrap_or(&self.origin)
    }

    /// Results are ordered by line and column. Repeats step past point and wrap.
    fn index_from(&self, cursor: &Vec2<usize>, include_cursor: bool) -> Option<usize> {
        let len = self.results.len();
        if len == 0 {
            return None;
        }
        let point = (cursor.y, cursor.x);
        let after_equal = match self.direction {
            SearchDirection::Forward => !include_cursor,
            SearchDirection::Backward => include_cursor,
        };
        let index = self.results.partition_point(|m| {
            let start = (m.line_index, m.columns.start);
            start < point || (after_equal && start == point)
        });
        Some(match self.direction {
            SearchDirection::Forward => index % len,
            SearchDirection::Backward => index.checked_sub(1).unwrap_or(len - 1),
        })
    }

    fn position(&self) -> Vec2<usize> {
        self.current_result_index.map_or_else(
            || self.anchor().clone(),
            |i| {
                let result = &self.results[i];
                Vec2::new(result.columns.start, result.line_index)
            },
        )
    }
}

impl Pager {
    pub(crate) fn begin_or_repeat_search(&mut self, direction: SearchDirection) -> io::Result<()> {
        if self.search.as_ref().is_some_and(|s| !s.results.is_empty()) {
            return self.repeat_search(direction);
        }
        self.search = Some(Search::new(self, direction));
        self.draw_status_line()
    }

    fn repeat_search(&mut self, direction: SearchDirection) -> io::Result<()> {
        let Some(search) = &mut self.search else {
            return Ok(());
        };
        search.direction = direction;
        if search.results.is_empty() {
            search.update_matches(&self.lines, self.settings.smartcase_search);
        } else {
            search.current_result_index =
                search.index_from(&Vec2::new(self.cursor_x, self.logical_y), false);
        }
        let position = search.position();
        if search.current_result_index.is_some() {
            search.repeat_anchor = Some(position.clone());
        }
        self.set_cursor(position.x, position.y);
        self.draw()
    }

    pub(crate) fn search_mode(&mut self, event: KeyEvent) -> io::Result<()> {
        let Some(search) = &mut self.search else {
            return Ok(());
        };
        let control = event.modifiers == KeyModifiers::CONTROL;
        if event.code == KeyCode::Esc
            || (control && matches!(event.code, KeyCode::Char('g' | 'c' | '[')))
        {
            let origin = search.origin.clone();
            let viewport = search.origin_viewport;
            let wish_x = search.origin_wish_x;
            self.search = None;
            self.set_cursor(origin.x, origin.y);
            self.viewport_start = viewport;
            self.wish_cursor_x = wish_x;
            return self.draw();
        }
        match event.code {
            KeyCode::Char('s') if control => return self.repeat_search(SearchDirection::Forward),
            KeyCode::Char('r') if control => return self.repeat_search(SearchDirection::Backward),
            KeyCode::Enter => {
                if search.results.is_empty() && search.error.is_none() {
                    search.update_matches(&self.lines, self.settings.smartcase_search);
                }
                search.state = SearchState::Submitted;
                let pos = search.position();
                self.set_cursor(pos.x, pos.y);
                return self.draw();
            }
            KeyCode::Backspace => {
                search.query.pop();
            }
            KeyCode::Char(c)
                if (event.modifiers - KeyModifiers::SHIFT).is_empty() && !c.is_control() =>
            {
                search.query.push(c);
            }
            _ => return Ok(()),
        }

        if self.settings.real_time_search {
            search.update_matches(&self.lines, self.settings.smartcase_search);
            let pos = search.position();
            self.set_cursor(pos.x, pos.y);
            self.draw()
        } else {
            let had_highlights = !search.results.is_empty();
            search.results.clear();
            search.current_result_index = None;
            search.error = None;
            if had_highlights {
                self.draw()
            } else {
                self.draw_status_line()
            }
        }
    }
}
