use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs},
    Frame,
};

use super::app::{App, JobStatus, View};

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
    let queue_count = app.queue.iter().filter(|q| q.status != JobStatus::Complete).count();
    let titles = vec![
        "Main".to_string(),
        format!("Queue ({})", queue_count),
        format!("Library ({})", app.library.len()),
    ];

    let selected = match app.view {
        View::Main | View::AddLink => 0,
        View::Queue => 1,
        View::Library => 2,
    };

    let portable_indicator = if app.portable_mode { " [P]" } else { "" };
    let title = format!(" rustwav{} ", portable_indicator);

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(title))
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

fn draw_main_view(frame: &mut Frame, app: &App, area: Rect) {
    let portable_status = if app.portable_mode {
        Span::styled("ON", Style::default().fg(Color::Green))
    } else {
        Span::styled("OFF", Style::default().fg(Color::DarkGray))
    };

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Welcome to rustwav!",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Keyboard shortcuts:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("    a", Style::default().fg(Color::Yellow)),
            Span::raw("  Add Spotify album"),
        ]),
        Line::from(vec![
            Span::styled("    p", Style::default().fg(Color::Yellow)),
            Span::raw("  Add Spotify playlist"),
        ]),
        Line::from(vec![
            Span::styled("    P", Style::default().fg(Color::Yellow)),
            Span::raw("  Toggle portable mode: "),
            portable_status,
        ]),
        Line::from(vec![
            Span::styled("  Tab", Style::default().fg(Color::Yellow)),
            Span::raw("  Switch views"),
        ]),
        Line::from(vec![
            Span::styled("    r", Style::default().fg(Color::Yellow)),
            Span::raw("  Refresh library"),
        ]),
        Line::from(vec![
            Span::styled("    q", Style::default().fg(Color::Yellow)),
            Span::raw("  Quit"),
        ]),
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
    if app.queue.is_empty() {
        let empty = Paragraph::new("  No downloads in queue.\n\n  Press 'a' to add album or 'p' for playlist.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Download Queue "));
        frame.render_widget(empty, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Queue list
            Constraint::Length(3), // Current progress
        ])
        .split(area);

    // Queue list
    let items: Vec<ListItem> = app
        .queue
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let (status_icon, status_color) = match &item.status {
                JobStatus::Pending => ("○", Color::DarkGray),
                JobStatus::Fetching => ("◐", Color::Yellow),
                JobStatus::Downloading => ("●", Color::Cyan),
                JobStatus::Complete => ("✓", Color::Green),
                JobStatus::Failed(_) => ("✗", Color::Red),
            };

            let progress_str = if item.progress.1 > 0 {
                format!(" [{}/{}]", item.progress.0, item.progress.1)
            } else {
                String::new()
            };

            let style = if i == app.queue_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let content = Line::from(vec![
                Span::styled(format!(" {} ", status_icon), Style::default().fg(status_color)),
                Span::raw(&item.name),
                Span::styled(progress_str, Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Download Queue "));

    frame.render_widget(list, chunks[0]);

    // Current download progress
    if let Some(current) = app.queue.iter().find(|q| q.status == JobStatus::Downloading) {
        let progress = if current.progress.1 > 0 {
            (current.progress.0 as f64 / current.progress.1 as f64).min(1.0)
        } else {
            0.0
        };

        let label = current.current_track.as_deref().unwrap_or("Processing...");

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(" Progress "))
            .gauge_style(Style::default().fg(Color::Cyan))
            .ratio(progress)
            .label(format!("{} ({}/{})", label, current.progress.0, current.progress.1));

        frame.render_widget(gauge, chunks[1]);
    } else {
        let idle = Paragraph::new("  Idle")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Progress "));
        frame.render_widget(idle, chunks[1]);
    }
}

fn draw_library_view(frame: &mut Frame, app: &App, area: Rect) {
    if app.library.is_empty() {
        let empty = Paragraph::new("  No tracks downloaded yet.\n\n  Add an album or playlist to get started!")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Library (0 tracks) "));
        frame.render_widget(empty, area);
        return;
    }

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

            let content = Line::from(vec![
                Span::styled("  ♪ ", Style::default().fg(Color::Cyan)),
                Span::styled(&track.artist, Style::default().fg(Color::Yellow)),
                Span::raw(" - "),
                Span::raw(&track.title),
            ]);

            ListItem::new(content).style(style)
        })
        .collect();

    let title = format!(" Library ({} tracks) ", app.library.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(list, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}
