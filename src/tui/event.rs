use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;

use super::app::{App, View};

pub fn handle_events(app: &mut App) -> anyhow::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if app.input_mode {
                handle_input_mode(app, key.code);
            } else {
                handle_normal_mode(app, key.code, key.modifiers);
            }
        }
    }
    Ok(())
}

fn handle_input_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => app.submit_input(),
        KeyCode::Esc => app.cancel_input(),
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        _ => {}
    }
}

fn handle_normal_mode(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.quit(),
        KeyCode::Tab => app.next_view(),
        KeyCode::Char('a') => app.start_add_album(),
        KeyCode::Char('p') => app.start_add_playlist(),
        KeyCode::Char('P') => app.toggle_portable(),
        KeyCode::Up | KeyCode::Char('k') => match app.view {
            View::Queue => app.queue_up(),
            View::Library => app.library_up(),
            _ => {}
        },
        KeyCode::Down | KeyCode::Char('j') => match app.view {
            View::Queue => app.queue_down(),
            View::Library => app.library_down(),
            _ => {}
        },
        KeyCode::Char('r') => app.refresh_library(),
        _ => {}
    }
}
