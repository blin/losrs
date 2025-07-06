use std::io::Write;
use std::io::stdout;

use anyhow::Result;
use anyhow::anyhow;

use crossterm::QueueableCommand;
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
    if is_escape_request(&key_event) {
        Err(anyhow!("Immediate nope out requested"))
    } else {
        Ok(key_event)
    }
}

const ESCAPE_INSTRUCTIONS: &str = "Ctrl+C or Esc to nope out";

pub fn wait_for_anykey(action_description: &str) -> Result<()> {
    print!("\nPress any key to {action_description} ({ESCAPE_INSTRUCTIONS})");
    stdout().flush()?;

    grab_key_event()?;

    Ok(())
}

#[derive(Debug)]
pub enum ReviewResponse {
    Remembered,
    Forgot,
}

pub fn wait_for_review() -> Result<ReviewResponse> {
    print!("\nRemembered? (y/N, {ESCAPE_INSTRUCTIONS})");
    stdout().flush()?;

    let key_event = grab_key_event()?;

    let response = match key_event.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => ReviewResponse::Remembered,
        _ => ReviewResponse::Forgot,
    };

    Ok(response)
}
