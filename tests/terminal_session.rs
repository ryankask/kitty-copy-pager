#![cfg(unix)]

use crossterm::{Command, clipboard::CopyToClipboard};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};

const ENTER_ALTERNATE_SCREEN: &[u8] = b"\x1b[?1049h";
const LEAVE_ALTERNATE_SCREEN: &[u8] = b"\x1b[?1049l";
const TEST_TIMEOUT: Duration = Duration::from_secs(3);

struct PagerOutcome {
    success: bool,
    entered_raw_mode: bool,
    restored_terminal_mode: bool,
    output: Vec<u8>,
}

fn run_pager(
    scrollback: &str,
    keystrokes: Option<&[u8]>,
    detach_controlling_terminal: bool,
) -> PagerOutcome {
    run_pager_with(
        scrollback,
        keystrokes,
        detach_controlling_terminal,
        (80, 24),
        &[],
    )
}

fn run_pager_with(
    scrollback: &str,
    keystrokes: Option<&[u8]>,
    detach_controlling_terminal: bool,
    (cols, rows): (u16, u16),
    args: &[&str],
) -> PagerOutcome {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open test PTY");

    let original_terminal_mode = pair.master.get_termios().expect("read PTY terminal mode");
    let mut writer = keystrokes.map(|_| pair.master.take_writer().expect("take PTY writer"));
    let mut reader = pair.master.try_clone_reader().expect("clone PTY reader");
    let reader_thread = thread::spawn(move || {
        let mut output = Vec::new();
        let _ = reader.read_to_end(&mut output);
        output
    });

    let shell_command = if detach_controlling_terminal {
        "printf '%s' \"$SCROLLBACK\" | setsid \"$PAGER_BIN\" \"$@\""
    } else {
        "printf '%s' \"$SCROLLBACK\" | \"$PAGER_BIN\" \"$@\""
    };
    let mut command = CommandBuilder::new("sh");
    command.args(["-c", shell_command, "pager-test"]);
    command.args(args);
    command.env("SCROLLBACK", scrollback);
    command.env("PAGER_BIN", env!("CARGO_BIN_EXE_kitty-copy-pager"));

    let mut child = pair
        .slave
        .spawn_command(command)
        .expect("spawn pager in test PTY");
    drop(pair.slave);

    let started = Instant::now();
    let mut entered_raw_mode = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll pager process") {
            break status;
        }

        let current_terminal_mode = pair.master.get_termios().expect("read PTY terminal mode");
        if current_terminal_mode != original_terminal_mode {
            entered_raw_mode = true;
            if let (Some(keys), Some(mut writer)) = (keystrokes, writer.take()) {
                writer.write_all(keys).expect("send pager keystrokes");
                writer.flush().expect("flush pager keystrokes");
            }
        }

        if started.elapsed() >= TEST_TIMEOUT {
            child.kill().expect("kill timed-out pager");
            panic!("pager did not exit within {TEST_TIMEOUT:?}");
        }
        thread::sleep(Duration::from_millis(10));
    };

    let restored_terminal_mode = pair
        .master
        .get_termios()
        .expect("read final PTY terminal mode")
        == original_terminal_mode;
    drop(writer);
    drop(pair.master);
    let output = reader_thread.join().expect("join PTY reader");

    PagerOutcome {
        success: status.success(),
        entered_raw_mode,
        restored_terminal_mode,
        output,
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn frames(output: &str) -> impl Iterator<Item = &str> {
    output.split("\x1b[?2026h").skip(1).map(|part| {
        part.split_once("\x1b[?2026l")
            .expect("synchronized repaint must finish")
            .0
    })
}

fn assert_copied(outcome: &PagerOutcome, expected: &str) {
    assert!(outcome.success && outcome.restored_terminal_mode);
    let mut command = String::new();
    CopyToClipboard::to_clipboard_from(expected)
        .write_ansi(&mut command)
        .unwrap();
    assert!(
        contains(&outcome.output, command.as_bytes()),
        "expected to copy {expected:?}, output: {:?}",
        String::from_utf8_lossy(&outcome.output)
    );
}

#[test]
fn empty_scrollback_exits_without_entering_the_alternate_screen() {
    let outcome = run_pager("", None, false);

    assert!(outcome.success, "pager output: {:?}", outcome.output);
    assert!(!contains(&outcome.output, ENTER_ALTERNATE_SCREEN));
}

#[test]
fn quitting_restores_the_screen_and_terminal_mode() {
    let outcome = run_pager("first line\nsecond line\n", Some(b"q"), false);

    assert!(outcome.success, "pager output: {:?}", outcome.output);
    assert!(outcome.entered_raw_mode);
    assert!(contains(&outcome.output, ENTER_ALTERNATE_SCREEN));
    assert!(contains(&outcome.output, LEAVE_ALTERNATE_SCREEN));
    assert!(outcome.restored_terminal_mode);
}

#[test]
fn moving_a_selection_leaves_other_rows_on_screen() {
    let text = "stationary row\nab";
    let outcome = run_pager(text, Some(b"\0\x02\r"), false);
    assert_copied(&outcome, "b");
    let output = String::from_utf8(outcome.output).unwrap();
    assert_eq!(
        output.matches("stationary row").count(),
        1,
        "unaffected row repainted"
    );
    let movement = frames(&output).last().unwrap();
    assert!(!movement.contains("\x1b[2J"));
    assert!(movement.contains("\x1b[2;2Hb"));
}

#[test]
fn shrinking_a_selection_restores_line_styles_search_matches_and_the_newline_cell() {
    let text = "\x1b[31mstationary\nab\nxy\x1b[0m\nfooter";
    let outcome = run_pager(text, Some(b"\x1b<\x13ab\r\0\x0e\x10q"), false);
    assert!(outcome.success && outcome.restored_terminal_mode);
    let output = String::from_utf8(outcome.output).unwrap();
    let frame = frames(&output).last().unwrap();
    assert!(!frame.contains("stationary") && !frame.contains("footer"));
    assert!(frame.contains("\x1b[31m\x1b[2;1Hab\x1b[3;1Hxy")); // inherited foreground
    assert_eq!(frame.matches("\x1b[2;1Hab").count(), 2); // base text and its search highlight
    assert!(frame.contains("\x1b[2;3H\x1b[K")); // remove the old selected newline
}

#[test]
fn scrolling_while_selecting_never_clears_the_display_between_frames() {
    let outcome = run_pager_with(
        "one\ntwo\nthree\nfour",
        Some(b"\0\x10\x10\x0e\r"),
        false,
        (12, 3),
        &[],
    );
    assert_copied(&outcome, "e\nfour");
    let output = String::from_utf8(outcome.output).unwrap();
    let (_, paints) = output.split_once("\x1b[?2026h").unwrap();
    assert!(!paints.contains("\x1b[2J"));
    let last = frames(&output).last().unwrap();
    assert!(last.contains("two") && last.contains("three"));
}

#[cfg(target_os = "linux")]
#[test]
fn setup_error_restores_the_screen() {
    let outcome = run_pager("line\n", None, true);

    assert!(!outcome.success);
    assert!(contains(&outcome.output, ENTER_ALTERNATE_SCREEN));
    assert!(contains(&outcome.output, LEAVE_ALTERNATE_SCREEN));
}

#[test]
fn meta_word_selection_crosses_whitespace_in_both_directions() {
    let text = "\x1b[32mone\x1b[0m   \n\n   \n  世界";
    let forward = run_pager(text, Some(b"\x1b<\x1bf\0\x1bf\x1bw"), false);
    assert_copied(&forward, "   \n\n   \n  世界");

    let backward = run_pager(text, Some(b"\x1bb\0\x1bb\x17"), false);
    assert_copied(&backward, "one   \n\n   \n  ");
}

#[test]
fn a_repeated_search_can_be_edited_before_accepting_a_region() {
    let keys = b"\x1b<\0\x13cat\x13al\r\x06\x1bw";
    for args in [&[][..], &["--disable-real-time-search"][..]] {
        let outcome = run_pager_with(
            "alpha cat\nbeta catalog\nend",
            Some(keys),
            false,
            (80, 24),
            args,
        );
        assert_copied(&outcome, "alpha cat\nbeta c");
        assert!(contains(&outcome.output, b"/catal"));
    }
}

#[test]
fn cancelling_a_search_restores_the_previous_region() {
    let keys = b"\x1b<\x06\0\x06\x13second\x07\r";
    let outcome = run_pager("first\nsecond", Some(keys), false);
    assert_copied(&outcome, "i");
}

#[test]
fn backward_regex_search_paints_exact_ranges_and_reports_the_chosen_match() {
    let outcome = run_pager("é aaa X a Z", Some(b"\x12a+\r\x0b"), false);
    assert_copied(&outcome, "a Z");
    let output = String::from_utf8(outcome.output).unwrap();
    let last_frame = frames(&output).last().unwrap();
    assert!(last_frame.contains("\x1b[1;3Haaa\x1b"));
    assert!(last_frame.contains("\x1b[1;9Ha\x1b"));
    assert!(last_frame.contains("[2/2]"));
}

#[test]
fn zero_width_matches_are_navigable_without_painting_a_character() {
    let outcome = run_pager("one\ntwo", Some(b"\x13^\r\x13\x0b"), false);
    assert_copied(&outcome, "two");
    let output = String::from_utf8(outcome.output).unwrap();
    let last_frame = frames(&output).last().unwrap();
    assert!(last_frame.contains("[2/2]"));
    // The base text is painted once. Neither empty match repaints its first character.
    assert_eq!(last_frame.matches("\x1b[1;1Ho").count(), 1);
    assert_eq!(last_frame.matches("\x1b[2;1Ht").count(), 1);
}

#[test]
fn a_short_viewport_keeps_the_line_being_copied_on_screen() {
    let text = "line 0\nline 1\nline 2\nline 3\nline 4\nline 5";
    let keys = b"\x1b<\x0e\x0b";
    let outcome = run_pager_with(text, Some(keys), false, (20, 4), &["--scrolloff", "100"]);
    assert_copied(&outcome, "line 1");
    let output = String::from_utf8(outcome.output).unwrap();
    let last_frame = frames(&output)
        .filter(|f| f.contains("line "))
        .last()
        .unwrap();
    assert!(last_frame.contains("\x1b[2;1Hline 1"));

    let one_row = run_pager_with(text, Some(keys), false, (20, 1), &[]);
    assert_copied(&one_row, "line 1");
    let output = String::from_utf8(one_row.output).unwrap();
    let last_frame = frames(&output).last().unwrap();
    assert!(last_frame.contains("\x1b[1;1Hline 1"));
    assert!(!last_frame.contains("Ln "));
}

#[test]
fn a_long_query_fits_a_narrow_prompt_and_still_finds_the_full_regex() {
    let query = "(?:(?:(?:漢字)))";
    let keys = format!("\x13{query}\r\x0b");
    for args in [&[][..], &["--disable-real-time-search"][..]] {
        let outcome = run_pager_with("xx漢字 tail", Some(keys.as_bytes()), false, (9, 3), args);
        assert_copied(&outcome, "漢字 tail");
        assert!(!contains(&outcome.output, format!("/{query}").as_bytes()));
    }
}

#[test]
fn tab_stops_use_terminal_cells_and_copy_without_formatting() {
    let outcome = run_pager_with(
        "\x1b[32m界\x1b[0m\tX\tY",
        Some(b"\x01\x0b"),
        false,
        (80, 24),
        &["--tab-width", "4"],
    );
    assert_copied(&outcome, "界  X   Y");
}

#[test]
fn wide_lines_keep_the_search_match_visible_and_copy_the_full_suffix() {
    let text = "\x1b[35m0123456789界tail\x1b[0m";
    let outcome = run_pager_with(text, Some("\x13界\r\x0b".as_bytes()), false, (6, 4), &[]);
    assert_copied(&outcome, "界tail");
    let output = String::from_utf8(outcome.output).unwrap();
    let last_frame = frames(&output).last().unwrap();
    assert!(last_frame.contains("\x1b[35m6789界\x1b[0m"));
    assert!(last_frame.contains("\x1b[1;5H界\x1b"));
}

#[test]
fn smart_case_and_its_override_select_the_expected_line() {
    let text = "Alpha first\nalpha second";
    let keys = b"\x1b<\x13alpha\r\x0b";
    assert_copied(&run_pager(text, Some(keys), false), "Alpha first");
    let sensitive = run_pager_with(
        text,
        Some(keys),
        false,
        (80, 24),
        &["--disable-smartcase-search"],
    );
    assert_copied(&sensitive, "alpha second");
}

#[test]
fn copying_an_empty_region_preserves_the_clipboard() {
    for keys in [&b"\x0b"[..], &b"\0\r"[..]] {
        let outcome = run_pager("last line", Some(keys), false);
        assert!(outcome.success && outcome.restored_terminal_mode);
        assert!(!contains(&outcome.output, b"\x1b]52;"));
    }
}
