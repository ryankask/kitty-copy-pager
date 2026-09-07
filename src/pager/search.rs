use super::Pager;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use regex::Regex;
use std::io::{self};
use unicode_width::UnicodeWidthStr;

pub(crate) struct SearchResult {
    pub line_index: usize,
    pub column_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchDirection {
    Forward,
    Backward,
}

#[derive(PartialEq, Clone)]
pub(crate) enum SearchState {
    Typing,
    PendingRedraw,
    Highlighted,
    Hidden,
}

pub(crate) struct Search {
    pub query: String,
    pub direction: SearchDirection,
    pub state: SearchState,
    pub results: Vec<SearchResult>,
    pub error: Option<String>,
    pub current_result_index: usize,
    pub long_search: bool,
    pub last_match_pos: Option<(usize, usize)>, // (column, line) - tracks last jumped-to match
}

impl Search {
    pub fn new(state: SearchState, direction: SearchDirection) -> Self {
        Search {
            query: String::new(),
            direction,
            state,
            results: Vec::new(),
            error: None,
            current_result_index: 0,
            long_search: false,
            last_match_pos: None,
        }
    }
    pub fn new_with_err(msg: &str, direction: SearchDirection) -> Self {
        Search {
            query: String::new(),
            direction,
            state: SearchState::Hidden,
            results: Vec::new(),
            error: Some(msg.to_string()),
            current_result_index: 0,
            long_search: false,
            last_match_pos: None,
        }
    }

    pub fn set_error(&mut self, msg: &str) {
        self.results.clear();
        self.state = SearchState::Hidden;
        self.error = Some(msg.to_string());
    }
}

impl Pager {
    /// Builds regex pattern with smartcase support.
    /// If settings.smartcase_search is enabled and query has no uppercase chars,
    /// prepends (?i) for case-insensitive matching.
    fn build_search_pattern(query: &str, smart_case: bool) -> String {
        if smart_case && !query.chars().any(|c| c.is_uppercase()) {
            format!("(?i){}", query)
        } else {
            query.to_string()
        }
    }

    pub(crate) fn begin_or_repeat_search(&mut self, direction: SearchDirection) -> io::Result<()> {
        let can_repeat = self.search.as_ref().is_some_and(|search| {
            matches!(
                search.state,
                SearchState::PendingRedraw | SearchState::Highlighted
            ) && search.error.is_none()
                && !search.results.is_empty()
        });

        if can_repeat {
            if let Some(search) = &mut self.search {
                search.direction = direction;
            }
            return match direction {
                SearchDirection::Forward => self.move_to_next_match(),
                SearchDirection::Backward => self.move_to_prev_match(),
            };
        }

        self.search = Some(Search::new(SearchState::Typing, direction));
        self.draw_status_line()
    }

    pub(crate) fn search_mode(&mut self, event: KeyEvent) -> io::Result<Option<SearchDirection>> {
        let get_lines = |query_width: usize, term_width: usize, long_search: bool| -> usize {
            if long_search {
                (query_width as f64 / term_width.saturating_sub(1) as f64).ceil() as usize
            } else {
                1
            }
        };

        let before_lines = if let Some(search) = &self.search
            && !self.settings.real_time_search
        {
            get_lines(search.query.width(), self.term_width, search.long_search)
        } else {
            1
        };

        let Some(search) = &mut self.search else {
            return Ok(None);
        };

        let control = event.modifiers == KeyModifiers::CONTROL;
        match event.code {
            KeyCode::Char('s') if control => {
                search.direction = SearchDirection::Forward;
                return Ok(Some(SearchDirection::Forward));
            }
            KeyCode::Char('r') if control => {
                search.direction = SearchDirection::Backward;
                return Ok(Some(SearchDirection::Backward));
            }
            KeyCode::Char('g') | KeyCode::Char('c') if control => {
                search.state = SearchState::Hidden;
                self.draw()?;
                return Ok(None);
            }
            KeyCode::Char(c)
                if !event.modifiers.intersects(
                    KeyModifiers::CONTROL
                        | KeyModifiers::ALT
                        | KeyModifiers::SUPER
                        | KeyModifiers::HYPER
                        | KeyModifiers::META,
                ) =>
            {
                search.query.push(c);
                if self.settings.real_time_search {
                    self.search_realtime();
                }
            }
            KeyCode::Backspace => {
                search.query.pop();
                if self.settings.real_time_search {
                    self.search_realtime();
                }
            }
            KeyCode::Esc => {
                search.state = SearchState::Hidden;
                self.draw()?;
                return Ok(None);
            }
            KeyCode::Enter => return Ok(Some(search.direction)),
            _ => {}
        }

        let after_lines = if let Some(search) = &self.search
            && !self.settings.real_time_search
        {
            get_lines(search.query.width(), self.term_width, search.long_search)
        } else {
            1
        };

        let needs_full_redraw = after_lines < before_lines;

        if self.settings.real_time_search || needs_full_redraw {
            self.draw()?;
        } else {
            self.draw_status_line()?;
        }
        Ok(None)
    }

    /// Performs real-time search while typing.
    /// Silently ignores regex errors (for incomplete patterns) and clears results instead.
    /// Note: Keeps state as Typing to stay in search mode.
    pub(crate) fn search_realtime(&mut self) {
        let Some(search) = &mut self.search else {
            return;
        };

        search.error = None;
        if search.query.is_empty() {
            search.results.clear();
            search.state = SearchState::Typing;
            return;
        }
        let pattern = Self::build_search_pattern(&search.query, self.settings.smartcase_search);
        match Regex::new(&pattern) {
            Ok(regex) => {
                search.results.clear();

                for (line_index, line) in self.lines.iter().enumerate() {
                    let display = line.display();
                    for mat in regex.find_iter(display) {
                        let column_index = display[..mat.start()].chars().count();
                        search.results.push(SearchResult {
                            line_index,
                            column_index,
                        });
                    }
                }
                search.state = SearchState::Typing;
            }
            Err(_) => {
                search.results.clear();
                search.state = SearchState::Typing;
            }
        }
    }

    pub(crate) fn search(&mut self) {
        let Some(search) = &mut self.search else {
            return;
        };

        search.state = SearchState::Hidden;
        if search.query.is_empty() {
            search.set_error("Error: empty search query");
            return;
        }

        let pattern = Self::build_search_pattern(&search.query, self.settings.smartcase_search);
        match Regex::new(&pattern) {
            Ok(regex) => {
                search.results.clear();

                for (line_index, line) in self.lines.iter().enumerate() {
                    let display = line.display();
                    for mat in regex.find_iter(display) {
                        let column_index = display[..mat.start()].chars().count();
                        search.results.push(SearchResult {
                            line_index,
                            column_index,
                        });
                    }
                }
                if search.results.is_empty() {
                    search.results.clear();
                    search.state = SearchState::Hidden;
                    search.error = Some("Error: could not find any occurrences".to_string());
                    return;
                }
            }
            Err(_) => {
                search.set_error("Error: invalid regex pattern");
                return;
            }
        }
        search.state = SearchState::PendingRedraw;
    }

    pub(crate) fn move_to_next_match(&mut self) -> io::Result<()> {
        let next = if let Some(search) = &self.search {
            if search.error.is_some() || search.results.is_empty() {
                None
            } else {
                let at_last_match = search
                    .last_match_pos
                    .is_some_and(|(col, line)| col == self.cursor_x && line == self.logical_y);

                if at_last_match {
                    let next_index = (search.current_result_index + 1) % search.results.len();
                    Some((
                        search.results[next_index].column_index,
                        search.results[next_index].line_index,
                        next_index,
                    ))
                } else {
                    self.get_closest_next_match()
                        .map(|(result, idx)| (result.column_index, result.line_index, idx))
                }
            }
        } else {
            None
        };

        match next {
            Some((col, line, idx)) => {
                if let Some(search) = &mut self.search {
                    search.current_result_index = idx;
                    search.state = SearchState::Highlighted;
                    search.last_match_pos = Some((col, line));
                }
                self.move_to(col, line)?;
            }
            None => {
                self.search = Some(Search::new_with_err(
                    "Error: No search query",
                    SearchDirection::Forward,
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn move_to_prev_match(&mut self) -> io::Result<()> {
        let prev = if let Some(search) = &self.search {
            if search.error.is_some() || search.results.is_empty() {
                None
            } else {
                let at_last_match = search
                    .last_match_pos
                    .is_some_and(|(col, line)| col == self.cursor_x && line == self.logical_y);

                if at_last_match {
                    let prev_index = search
                        .current_result_index
                        .checked_sub(1)
                        .unwrap_or(search.results.len() - 1);
                    Some((
                        search.results[prev_index].column_index,
                        search.results[prev_index].line_index,
                        prev_index,
                    ))
                } else {
                    self.get_closest_prev_match()
                        .map(|(result, idx)| (result.column_index, result.line_index, idx))
                }
            }
        } else {
            None
        };

        match prev {
            Some((col, line, idx)) => {
                if let Some(search) = &mut self.search {
                    search.current_result_index = idx;
                    search.state = SearchState::Highlighted;
                    search.last_match_pos = Some((col, line));
                }
                self.move_to(col, line)?;
            }
            None => {
                self.search = Some(Search::new_with_err(
                    "Error: No search query",
                    SearchDirection::Backward,
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn get_closest_next_match(&self) -> Option<(&SearchResult, usize)> {
        if let Some(search) = &self.search {
            if search.error.is_some() || search.results.is_empty() {
                return None;
            }
            for (i, result) in search.results.iter().enumerate() {
                if result.line_index > self.logical_y
                    || (result.line_index == self.logical_y && result.column_index > self.cursor_x)
                {
                    return Some((result, i));
                }
            }
            return Some((&search.results[0], 0));
        }
        None
    }

    pub(crate) fn get_closest_prev_match(&self) -> Option<(&SearchResult, usize)> {
        if let Some(search) = &self.search {
            if search.error.is_some() || search.results.is_empty() {
                return None;
            }
            for (i, result) in search.results.iter().enumerate().rev() {
                if result.line_index < self.logical_y
                    || (result.line_index == self.logical_y && result.column_index < self.cursor_x)
                {
                    return Some((result, i));
                }
            }
            return Some((
                &search.results[search.results.len() - 1],
                search.results.len() - 1,
            ));
        }
        None
    }

    pub(crate) fn move_to_closest_match(&mut self, next: bool) -> io::Result<()> {
        let closest_match = if next {
            self.get_closest_next_match()
        } else {
            self.get_closest_prev_match()
        };
        match closest_match {
            Some((result, idx)) => {
                let col = result.column_index;
                let line = result.line_index;
                self.move_to(col, line)?;
                if let Some(search) = &mut self.search {
                    search.current_result_index = idx;
                    search.state = SearchState::Highlighted;
                    search.last_match_pos = Some((col, line));
                }
            }
            None => {
                self.search = Some(Search::new_with_err(
                    "Error: No search query",
                    if next {
                        SearchDirection::Forward
                    } else {
                        SearchDirection::Backward
                    },
                ));
                self.draw_status_line()?;
            }
        }
        Ok(())
    }
}
