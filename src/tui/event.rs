use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;

use super::app::{App, View};

pub fn handle_events(app: &mut App) -> anyhow::Result<()> {
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            if app.input_mode {
                handle_input_mode(app, key.code);
            } else {
                match app.view {
                    View::LinkSettings => handle_settings_mode(app, key.code),
                    View::Logs => handle_logs_mode(app, key.code, key.modifiers),
                    View::M3UConfirm => handle_m3u_confirm_mode(app, key.code),
                    View::ConvertSettings => handle_convert_settings_mode(app, key.code),
                    View::ConvertConfirm => handle_convert_confirm_mode(app, key.code),
                    View::ConvertBatchConfirm => handle_convert_batch_confirm_mode(app, key.code),
                    View::CleanupConfirm => handle_cleanup_confirm_mode(app, key.code),
                    View::ErrorLog => handle_error_log_mode(app, key.code, key.modifiers),
                    _ => handle_normal_mode(app, key.code, key.modifiers),
                }
            }
        }
    }
    Ok(())
}

fn handle_input_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => {
            if app.view == View::GenerateM3U {
                app.submit_m3u_input();
            } else {
                app.submit_input();
            }
        }
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

fn handle_settings_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => app.submit_settings(),
        KeyCode::Esc => app.cancel_settings(),
        KeyCode::Up | KeyCode::Char('k') => app.settings_up(),
        KeyCode::Down | KeyCode::Char('j') => app.settings_down(),
        KeyCode::Left | KeyCode::Char('h') => app.settings_left(),
        KeyCode::Right | KeyCode::Char('l') => app.settings_right(),
        _ => {}
    }
}

fn handle_m3u_confirm_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter | KeyCode::Char('y') => app.confirm_m3u(),
        KeyCode::Esc | KeyCode::Char('n') => app.cancel_m3u(),
        _ => {}
    }
}

fn handle_logs_mode(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.quit(),
        KeyCode::Tab => app.next_view(),
        KeyCode::Up | KeyCode::Char('k') => app.logs_up(),
        KeyCode::Down | KeyCode::Char('j') => app.logs_down(),
        KeyCode::Char('g') => app.logs_top(),
        KeyCode::Char('G') => app.logs_bottom(),
        KeyCode::Home => app.logs_top(),
        KeyCode::End => app.logs_bottom(),
        // Allow switching to add album/playlist from logs view
        KeyCode::Char('a') => app.start_add_album(),
        KeyCode::Char('p') => app.start_add_playlist(),
        KeyCode::Char('y') => app.start_add_youtube_playlist(),
        KeyCode::Char('P') => app.toggle_portable(),
        KeyCode::Char('r') => app.refresh_library(),
        KeyCode::Char('m') => app.start_generate_m3u(),
        KeyCode::Char(' ') => app.toggle_pause(),
        _ => {}
    }
}

fn handle_normal_mode(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.quit(),
        // 'c' in Library view starts conversion for selected track
        KeyCode::Char('c') if app.view == View::Library => app.start_convert(),
        // 'C' in Library view starts conversion for ALL tracks
        KeyCode::Char('C') if app.view == View::Library => app.start_convert_all(),
        // 'x' in Library view refreshes metadata for selected track
        KeyCode::Char('x') if app.view == View::Library => app.start_refresh_metadata(),
        // 'X' in Library view refreshes metadata for ALL tracks
        KeyCode::Char('X') if app.view == View::Library => app.start_refresh_all_metadata(),
        // 'z' in Library view starts database cleanup
        KeyCode::Char('z') if app.view == View::Library => app.start_cleanup_database(),
        KeyCode::Tab => app.next_view(),
        KeyCode::Char('a') => app.start_add_album(),
        KeyCode::Char('p') => app.start_add_playlist(),
        KeyCode::Char('y') => app.start_add_youtube_playlist(),
        KeyCode::Char('P') => app.toggle_portable(),
        KeyCode::Char('l') => app.show_logs(),
        KeyCode::Char('e') => app.show_error_log(),
        KeyCode::Char('m') => app.start_generate_m3u(),
        KeyCode::Char(' ') => app.toggle_pause(),
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

fn handle_convert_settings_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Enter => app.submit_convert(),
        KeyCode::Esc => app.cancel_convert(),
        KeyCode::Left => app.convert_settings_left(),
        KeyCode::Right => app.convert_settings_right(),
        KeyCode::Char('h') => app.convert_quality_left(),
        KeyCode::Char('l') => app.convert_quality_right(),
        KeyCode::Char(' ') => app.convert_toggle_refresh(),
        _ => {}
    }
}

fn handle_convert_confirm_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('y') => app.confirm_delete_original(),
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_delete_original(),
        _ => {}
    }
}

fn handle_convert_batch_confirm_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('y') => app.confirm_batch_delete_originals(),
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_batch_delete_originals(),
        _ => {}
    }
}

fn handle_cleanup_confirm_mode(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('y') => app.confirm_cleanup(),
        KeyCode::Char('n') | KeyCode::Esc => app.cancel_cleanup(),
        _ => {}
    }
}

fn handle_error_log_mode(app: &mut App, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('q') => app.quit(),
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => app.quit(),
        KeyCode::Esc => {
            app.view = View::Main;
            app.status_message = "Returned to main view".to_string();
        }
        // Date navigation
        KeyCode::Left | KeyCode::Char('h') => app.error_date_prev(),
        KeyCode::Right | KeyCode::Char('l') => app.error_date_next(),
        // Tab navigation
        KeyCode::Tab => app.error_tab_next(),
        KeyCode::BackTab => app.error_tab_prev(),
        // Error list navigation
        KeyCode::Up | KeyCode::Char('k') => app.error_up(),
        KeyCode::Down | KeyCode::Char('j') => app.error_down(),
        // Delete selected error
        KeyCode::Char('d') => app.delete_selected_error(),
        // Clear all errors for current date
        KeyCode::Char('D') => app.clear_current_date_errors(),
        // Refresh
        KeyCode::Char('r') => {
            app.refresh_error_logs();
            app.status_message = "Error logs refreshed".to_string();
        }
        _ => {}
    }
}
