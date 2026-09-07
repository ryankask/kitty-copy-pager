use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    BackToIndentation,
    BackwardChar,
    BeginSelection,
    BeginningOfBuffer,
    BeginningOfLine,
    ClearSelection,
    CopyEndOfLine,
    CopySelection,
    EndOfBuffer,
    EndOfLine,
    ForwardChar,
    NextLine,
    NextWordEnd,
    PageDown,
    PageUp,
    PreviousLine,
    PreviousWord,
    Quit,
    Recenter,
    SearchBackward,
    SearchForward,
}

pub(crate) fn action_for(event: KeyEvent) -> Option<Action> {
    use Action::*;

    let modifiers = event.modifiers;
    let control = modifiers == KeyModifiers::CONTROL;
    let alt = modifiers == KeyModifiers::ALT;
    // With the enhanced keyboard protocol kitty reports the Shift used to
    // produce '<' and '>'. Legacy terminals report only Alt.
    let alt_punctuation = alt || modifiers == KeyModifiers::ALT | KeyModifiers::SHIFT;

    match event.code {
        KeyCode::Char('a') if control => Some(BeginningOfLine),
        KeyCode::Char('b') if control => Some(BackwardChar),
        KeyCode::Char('b') if alt => Some(PreviousWord),
        KeyCode::Char('c') if control => Some(Quit),
        KeyCode::Char('e') if control => Some(EndOfLine),
        KeyCode::Char('f') if control => Some(ForwardChar),
        KeyCode::Char('f') if alt => Some(NextWordEnd),
        KeyCode::Char('g') if control => Some(ClearSelection),
        KeyCode::Char('k') if control => Some(CopyEndOfLine),
        KeyCode::Char('l') if control => Some(Recenter),
        KeyCode::Char('m') if alt => Some(BackToIndentation),
        KeyCode::Char('n') if control => Some(NextLine),
        KeyCode::Char('p') if control => Some(PreviousLine),
        KeyCode::Char('r') if control => Some(SearchBackward),
        KeyCode::Char('s') if control => Some(SearchForward),
        KeyCode::Char('v') if control => Some(PageDown),
        KeyCode::Char('v') if alt => Some(PageUp),
        KeyCode::Char('w') if control || alt => Some(CopySelection),
        KeyCode::Char(' ') if control => Some(BeginSelection),
        KeyCode::Char('[') if control => Some(Quit),
        KeyCode::Char('<') if alt_punctuation => Some(BeginningOfBuffer),
        KeyCode::Char('>') if alt_punctuation => Some(EndOfBuffer),

        KeyCode::Char('q') if modifiers.is_empty() => Some(Quit),
        KeyCode::Char(' ') if modifiers.is_empty() => Some(PageDown),
        KeyCode::Esc => Some(Quit),
        KeyCode::Enter => Some(CopySelection),

        KeyCode::Left if modifiers.is_empty() => Some(BackwardChar),
        KeyCode::Right if modifiers.is_empty() => Some(ForwardChar),
        KeyCode::Up if modifiers.is_empty() => Some(PreviousLine),
        KeyCode::Down if modifiers.is_empty() => Some(NextLine),
        KeyCode::Home if control => Some(BeginningOfBuffer),
        KeyCode::End if control => Some(EndOfBuffer),
        KeyCode::Home if modifiers.is_empty() => Some(BeginningOfLine),
        KeyCode::End if modifiers.is_empty() => Some(EndOfLine),
        KeyCode::PageUp if modifiers.is_empty() => Some(PageUp),
        KeyCode::PageDown if modifiers.is_empty() => Some(PageDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn maps_emacs_character_and_line_movement() {
        assert_eq!(
            action_for(key(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            Some(Action::BackwardChar)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('f'), KeyModifiers::CONTROL)),
            Some(Action::ForwardChar)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('p'), KeyModifiers::CONTROL)),
            Some(Action::PreviousLine)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('n'), KeyModifiers::CONTROL)),
            Some(Action::NextLine)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            Some(Action::BeginningOfLine)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            Some(Action::EndOfLine)
        );
    }

    #[test]
    fn distinguishes_meta_word_movement_from_control_movement() {
        assert_eq!(
            action_for(key(KeyCode::Char('b'), KeyModifiers::ALT)),
            Some(Action::PreviousWord)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('f'), KeyModifiers::ALT)),
            Some(Action::NextWordEnd)
        );
    }

    #[test]
    fn maps_selection_and_copy_keys() {
        assert_eq!(
            action_for(key(KeyCode::Char(' '), KeyModifiers::CONTROL)),
            Some(Action::BeginSelection)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('w'), KeyModifiers::ALT)),
            Some(Action::CopySelection)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            Some(Action::ClearSelection)
        );
    }

    #[test]
    fn maps_search_keys() {
        assert_eq!(
            action_for(key(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            Some(Action::SearchForward)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Some(Action::SearchBackward)
        );
    }

    #[test]
    fn accepts_both_terminal_encodings_for_meta_angle_brackets() {
        assert_eq!(
            action_for(key(KeyCode::Char('<'), KeyModifiers::ALT)),
            Some(Action::BeginningOfBuffer)
        );
        assert_eq!(
            action_for(key(
                KeyCode::Char('>'),
                KeyModifiers::ALT | KeyModifiers::SHIFT
            )),
            Some(Action::EndOfBuffer)
        );
    }

    #[test]
    fn maps_page_and_buffer_movement() {
        assert_eq!(
            action_for(key(KeyCode::Char('v'), KeyModifiers::CONTROL)),
            Some(Action::PageDown)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('v'), KeyModifiers::ALT)),
            Some(Action::PageUp)
        );
        assert_eq!(
            action_for(key(KeyCode::Home, KeyModifiers::CONTROL)),
            Some(Action::BeginningOfBuffer)
        );
        assert_eq!(
            action_for(key(KeyCode::End, KeyModifiers::CONTROL)),
            Some(Action::EndOfBuffer)
        );
    }

    #[test]
    fn maps_copy_and_exit_shortcuts() {
        assert_eq!(
            action_for(key(KeyCode::Char('k'), KeyModifiers::CONTROL)),
            Some(Action::CopyEndOfLine)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Some(Action::CopySelection)
        );
        assert_eq!(
            action_for(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert_eq!(
            action_for(key(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Action::Quit)
        );
    }

    #[test]
    fn leaves_unmodified_text_keys_unbound() {
        assert_eq!(
            action_for(key(KeyCode::Char('f'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(
            action_for(key(KeyCode::Char('v'), KeyModifiers::NONE)),
            None
        );
    }
}
