#![cfg(unix)]

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
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
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
        "printf '%s' \"$SCROLLBACK\" | setsid \"$PAGER_BIN\""
    } else {
        "printf '%s' \"$SCROLLBACK\" | \"$PAGER_BIN\""
    };
    let mut command = CommandBuilder::new("sh");
    command.args(["-c", shell_command]);
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

#[cfg(target_os = "linux")]
#[test]
fn setup_error_restores_the_screen() {
    let outcome = run_pager("line\n", None, true);

    assert!(!outcome.success);
    assert!(contains(&outcome.output, ENTER_ALTERNATE_SCREEN));
    assert!(contains(&outcome.output, LEAVE_ALTERNATE_SCREEN));
}
