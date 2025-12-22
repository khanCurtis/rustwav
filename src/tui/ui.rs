use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

use super::app::{App, DownloadStatus, View};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header/tabs
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_main(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["Main", "Queue", "Library"];
    let selected = match app.view {
        View::Main | View::AddLink => 0,
        View::Queue => 1,
        View::Library => 2,
    };

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" rustwav "))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    frame.render_widget(tabs, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Main => draw_main_view(frame, app, area),
        View::AddLink => draw_add_link_view(frame, app, area),
        View::Queue => draw_queue_view(frame, app, area),
        View::Library => draw_library_view(frame, app, area),
    }
}

fn draw_main_view(frame: &mut Frame, _app: &App, area: Rect) {
    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Welcome to rustwav!",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Keyboard shortcuts:"),
        Line::from(""),
        Line::from(Span::styled("    a", Style::default().fg(Color::Yellow))),
        Line::from("      Add Spotify album"),
        Line::from(""),
        Line::from(Span::styled("    p", Style::default().fg(Color::Yellow))),
        Line::from("      Add Spotify playlist"),
        Line::from(""),
        Line::from(Span::styled("    Tab", Style::default().fg(Color::Yellow))),
        Line::from("      Switch views"),
        Line::from(""),
        Line::from(Span::styled("    q", Style::default().fg(Color::Yellow))),
        Line::from("      Quit"),
    ];

    let paragraph = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title(" Home "));

    frame.render_widget(paragraph, area);
}

fn draw_add_link_view(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .margin(2)
        .split(area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" Spotify Link "));

    frame.render_widget(input, chunks[0]);

    let help = Paragraph::new("Press Enter to submit, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help, chunks[1]);

    // Show cursor
    frame.set_cursor_position((
        chunks[0].x + app.input.len() as u16 + 1,
        chunks[0].y + 1,
    ));
}

fn draw_queue_view(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .queue
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let status_icon = match &item.status {
                DownloadStatus::Pending => "⏳",
                DownloadStatus::Downloading => "⬇️",
                DownloadStatus::Complete => "✓",
                DownloadStatus::Failed(_) => "✗",
            };

            let style = if i == app.queue_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let content = format!("{} {} - {}", status_icon, item.artist, item.title);
            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Download Queue "));

    if app.queue.is_empty() {
        let empty = Paragraph::new("  No downloads in queue. Press 'a' or 'p' to add.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Download Queue "));
        frame.render_widget(empty, area);
    } else {
        frame.render_widget(list, area);
    }
}

fn draw_library_view(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .library
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let style = if i == app.library_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let content = format!("  {} - {}", track.artist, track.title);
            ListItem::new(content).style(style)
        })
        .collect();

    let title = format!(" Library ({} tracks) ", app.library.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));

    if app.library.is_empty() {
        let empty = Paragraph::new("  No tracks downloaded yet.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Library (0 tracks) "));
        frame.render_widget(empty, area);
    } else {
        frame.render_widget(list, area);
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}
