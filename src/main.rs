use clap::Parser;
use crossterm::{
    cursor::{MoveTo, Show},
    event::{Event, poll, read},
    execute,
    style::ResetColor,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use kitty_copy_pager::pager::Pager;
use kitty_copy_pager::settings::*;
use std::io::{self, Write, stdout};
use std::time::Duration;

use log::*;
use simplelog::*;

const LOGGING_ENABLED: bool = false;

const INPUT_POLLING_RATE: u64 = 100;

struct TerminalSession {
    alternate_screen: bool,
    raw_mode: bool,
}

impl TerminalSession {
    fn enter() -> io::Result<Self> {
        let mut session = Self {
            alternate_screen: false,
            raw_mode: false,
        };

        session.alternate_screen = true;
        execute!(stdout(), EnterAlternateScreen, Clear(ClearType::All))?;
        enable_raw_mode()?;
        session.raw_mode = true;
        execute!(stdout(), MoveTo(0, 0))?;
        stdout().flush()?;

        Ok(session)
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.raw_mode {
            let _ = disable_raw_mode();
        }
        if self.alternate_screen {
            let _ = execute!(stdout(), ResetColor, Show, LeaveAlternateScreen);
        }
    }
}

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> io::Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    let args = Args::parse();
    let settings = Settings::from_args(args);
    let mut sb = Pager::new(settings)?;
    if sb.is_empty() {
        return Ok(());
    }

    if LOGGING_ENABLED {
        WriteLogger::init(
            LevelFilter::Debug,
            Config::default(),
            std::fs::File::create("debug.log").unwrap(),
        )
        .unwrap();
    }

    let _terminal = TerminalSession::enter()?;

    sb.draw()?;
    sb.draw_status_line()?;

    loop {
        if poll(Duration::from_millis(INPUT_POLLING_RATE))? {
            let event = read()?;
            if let Event::Key(e) = event {
                let quit = sb.handle_key_event(e)?;
                if quit {
                    break;
                }
            }
        }
    }

    Ok(())
}
