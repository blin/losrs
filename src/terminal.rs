use std::io::Write;
use std::io::stdout;

use anyhow::Result;
use anyhow::anyhow;

use crossterm::QueueableCommand;
use crossterm::cursor::MoveTo;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::{self};
use crossterm::terminal::Clear;
use crossterm::terminal::ClearType;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;

pub fn clear_screen() -> Result<()> {
    let mut stdout = stdout();
    stdout.queue(MoveTo(0, 0))?;
    stdout.queue(Clear(ClearType::All))?;
    stdout.flush()?;
    Ok(())
}

fn is_escape_request(key_event: &KeyEvent) -> bool {
    match key_event.code {
        KeyCode::Esc => true,
        KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

struct RawModeGuard {}

impl RawModeGuard {
    fn install() -> Self {
        enable_raw_mode().expect("I do not yet understand how this can fail...");
        RawModeGuard {}
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        disable_raw_mode().expect("I do not yet understand how this can fail...");
    }
}

#[derive(Debug)]
pub struct NopeOutError();

impl std::fmt::Display for NopeOutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Immediate nope out requested")
    }
}
impl std::error::Error for NopeOutError {}

fn grab_key_event() -> Result<KeyEvent> {
    let event: Event;
    {
        let _raw_mode_guard = RawModeGuard::install();
        event = event::read()?;
    }
    println!();
    let key_event = match event {
        Event::Key(key_event) => key_event,
        _ => return Err(anyhow!("expected a key event, got {:?}", event)),
    };
    if is_escape_request(&key_event) { Err((NopeOutError {}).into()) } else { Ok(key_event) }
}

const ESCAPE_INSTRUCTIONS: &str = "Ctrl+C or Esc to nope out";

#[derive(Debug, PartialEq)]
pub enum PreReviewResponse {
    ShowResponse,
    DelayReview,
}

pub fn wait_for_prereview() -> Result<PreReviewResponse> {
    print!("\nSpace - show the response; D - delay review by 1 day; {ESCAPE_INSTRUCTIONS}");
    stdout().flush()?;

    // BUG: key grabbing should be inside the while loop
    let key_event = grab_key_event()?;

    let mut tries = 3;

    while tries > 0 {
        match key_event.code {
            KeyCode::Char(' ') => return Ok(PreReviewResponse::ShowResponse),
            KeyCode::Char('d') => return Ok(PreReviewResponse::DelayReview),
            _ => tries -= 1,
        };
    }

    Err(anyhow!("Did not receive expected answer, aborting"))
}

#[derive(Debug, PartialEq)]
pub enum ReviewResponse {
    LittleEffort,
    SomeEffort,
    MuchEffort,
    NoRecall,
}

pub fn wait_for_review() -> Result<ReviewResponse> {
    print!(
        r#"
How much effort did recall require?
(1 - Little Effort; 2 - Some effort; 3 - Much Effort; 4 - Did not recall; {ESCAPE_INSTRUCTIONS})"#
    );
    stdout().flush()?;

    let key_event = grab_key_event()?;

    let mut tries = 3;

    while tries > 0 {
        match key_event.code {
            KeyCode::Char('1') => return Ok(ReviewResponse::LittleEffort),
            KeyCode::Char('2') => return Ok(ReviewResponse::SomeEffort),
            KeyCode::Char('3') => return Ok(ReviewResponse::MuchEffort),
            KeyCode::Char('4') => return Ok(ReviewResponse::NoRecall),
            _ => tries -= 1,
        };
    }

    Err(anyhow!("Did not receive expected answer, aborting"))
}

const DEFAULT_TERM_SIZE: (u16, u16) = (80, 24);

// (columns, lines)
pub fn grab_term_size() -> (u16, u16) {
    match crossterm::terminal::size() {
        Ok(s) => s,
        Err(_) => DEFAULT_TERM_SIZE,
    }
}
