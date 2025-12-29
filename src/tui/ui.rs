use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs},
    Frame,
};

use super::app::{App, JobStatus, SettingsField, View, FORMAT_OPTIONS, QUALITY_OPTIONS};

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
    let queue_count = app
        .queue
        .iter()
        .filter(|q| q.status != JobStatus::Complete)
        .count();
    let titles = vec![
        "Main".to_string(),
        format!("Queue ({})", queue_count),
        format!("Library ({})", app.library.len()),
        format!("Logs ({})", app.download_logs.len()),
    ];

    let selected = match app.view {
        View::Main | View::AddLink | View::LinkSettings | View::GenerateM3U | View::M3UConfirm => 0,
        View::Queue => 1,
        View::Library | View::ConvertSettings | View::ConvertConfirm | View::ConvertBatchConfirm => 2,
        View::Logs => 3,
    };

    let portable_indicator = if app.portable_mode { " [P]" } else { "" };
    let pause_indicator = if app.paused { " [PAUSED]" } else { "" };
    let title = format!(" rustwav{}{} ", portable_indicator, pause_indicator);

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(title))
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.view {
        View::Main => draw_main_view(frame, app, area),
        View::AddLink => draw_add_link_view(frame, app, area),
        View::LinkSettings => draw_link_settings_view(frame, app, area),
        View::Queue => draw_queue_view(frame, app, area),
        View::Library => draw_library_view(frame, app, area),
        View::Logs => draw_logs_view(frame, app, area),
        View::GenerateM3U => draw_generate_m3u_view(frame, app, area),
        View::M3UConfirm => draw_m3u_confirm_view(frame, app, area),
        View::ConvertSettings => draw_convert_settings_view(frame, app, area),
        View::ConvertConfirm => draw_convert_confirm_view(frame, app, area),
        View::ConvertBatchConfirm => draw_convert_batch_confirm_view(frame, app, area),
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
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
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
            Span::styled("    l", Style::default().fg(Color::Yellow)),
            Span::raw("  View download logs"),
        ]),
        Line::from(vec![
            Span::styled("    m", Style::default().fg(Color::Yellow)),
            Span::raw("  Generate M3U from Spotify link"),
        ]),
        Line::from(vec![
            Span::styled("Space", Style::default().fg(Color::Yellow)),
            Span::raw("  Pause/resume downloads"),
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

    let paragraph =
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Home "));

    frame.render_widget(paragraph, area);
}

fn draw_add_link_view(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Spotify Link "),
        );

    frame.render_widget(input, chunks[0]);

    let help = Paragraph::new("Press Enter to continue to settings, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help, chunks[1]);

    // Show cursor
    frame.set_cursor_position((chunks[0].x + app.input.len() as u16 + 1, chunks[0].y + 1));
}

fn draw_link_settings_view(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Download Settings ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Spacing
            Constraint::Length(2), // Format row
            Constraint::Length(2), // Quality row
            Constraint::Length(2), // Spacing
            Constraint::Min(0),    // Help text
        ])
        .margin(1)
        .split(inner);

    // Format selection
    let format_active = app.settings_field == SettingsField::Format;
    let format_label_style = if format_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let mut format_spans = vec![Span::styled("  Format:   ", format_label_style)];

    for (i, fmt) in FORMAT_OPTIONS.iter().enumerate() {
        let is_selected = i == app.selected_format;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if format_active {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        format_spans.push(Span::styled(format!(" {} ", fmt), style));
    }

    // Show portable mode note
    if app.portable_mode {
        format_spans.push(Span::styled(
            "  (portable: mp3 only)",
            Style::default().fg(Color::Yellow),
        ));
    }

    let format_line = Paragraph::new(Line::from(format_spans));
    frame.render_widget(format_line, chunks[1]);

    // Quality selection
    let quality_active = app.settings_field == SettingsField::Quality;
    let quality_label_style = if quality_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let mut quality_spans = vec![Span::styled("  Quality:  ", quality_label_style)];

    for (i, q) in QUALITY_OPTIONS.iter().enumerate() {
        let is_selected = i == app.selected_quality;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if quality_active {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        quality_spans.push(Span::styled(format!(" {} ", q), style));
    }

    let quality_line = Paragraph::new(Line::from(quality_spans));
    frame.render_widget(quality_line, chunks[2]);

    // Help text
    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw("  Select option    "),
            Span::styled("←/→", Style::default().fg(Color::Yellow)),
            Span::raw("  Change value"),
        ]),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Yellow)),
            Span::raw("  Start download   "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw("  Cancel"),
        ]),
    ];

    let help = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn draw_queue_view(frame: &mut Frame, app: &App, area: Rect) {
    if app.queue.is_empty() {
        let empty = Paragraph::new(
            "  No downloads in queue.\n\n  Press 'a' to add album or 'p' for playlist.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Download Queue "),
        );
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
                Span::styled(
                    format!(" {} ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::raw(&item.name),
                Span::styled(progress_str, Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Download Queue "),
    );

    frame.render_widget(list, chunks[0]);

    // Current download progress
    if let Some(current) = app
        .queue
        .iter()
        .find(|q| q.status == JobStatus::Downloading)
    {
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
            .label(format!(
                "{} ({}/{})",
                label, current.progress.0, current.progress.1
            ));

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
        let empty = Paragraph::new(
            "  No tracks downloaded yet.\n\n  Add an album or playlist to get started!",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Library (0 tracks) "),
        );
        frame.render_widget(empty, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

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

    let title = format!(" Library ({} tracks) - 'c' convert, 'C' convert all ", app.library.len());
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(list, chunks[0]);

    // Help hint at bottom
    let help = Paragraph::new(" ↑/↓ Navigate  |  c Convert  |  C Convert All  |  r Refresh  |  Tab Switch view")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[1]);
}

fn draw_logs_view(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Download Logs ({}) ", app.download_logs.len()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.download_logs.is_empty() {
        let empty = Paragraph::new("  No logs yet. Start a download to see output here.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, inner);
        return;
    }

    // Calculate visible lines
    let visible_height = inner.height as usize;
    let total_logs = app.download_logs.len();

    // Determine scroll position
    let start_idx = if total_logs <= visible_height {
        0
    } else {
        app.log_scroll
            .min(total_logs.saturating_sub(visible_height))
    };

    let items: Vec<ListItem> = app
        .download_logs
        .iter()
        .skip(start_idx)
        .take(visible_height)
        .map(|line| {
            // Color code different log types
            let style = if line.contains("ERROR") || line.contains("FAILED") {
                Style::default().fg(Color::Red)
            } else if line.contains("Complete") || line.contains("Finished") {
                Style::default().fg(Color::Green)
            } else if line.contains("Skipped") {
                Style::default().fg(Color::Yellow)
            } else if line.contains("Downloading") || line.contains("[download]") {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(Span::styled(format!(" {}", line), style)))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner);

    // Scroll indicator
    if total_logs > visible_height {
        let scroll_info = format!(
            " [{}-{}/{}] {}",
            start_idx + 1,
            (start_idx + visible_height).min(total_logs),
            total_logs,
            if app.log_auto_scroll {
                "[auto-scroll]"
            } else {
                "[manual]"
            }
        );
        let scroll_indicator = Paragraph::new(scroll_info)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Right);

        // Draw at bottom right of the area
        let indicator_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width - 1,
            height: 1,
        };
        frame.render_widget(scroll_indicator, indicator_area);
    }
}

fn draw_generate_m3u_view(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .margin(2)
        .split(area);

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Spotify Link (Album or Playlist) "),
        );

    frame.render_widget(input, chunks[0]);

    let help_text = if app.m3u_generating {
        "Fetching tracks from Spotify..."
    } else {
        "Enter a Spotify album/playlist link. Press Enter to generate M3U, Esc to cancel."
    };

    let help = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help, chunks[1]);

    // Show cursor
    if app.input_mode {
        frame.set_cursor_position((chunks[0].x + app.input.len() as u16 + 1, chunks[0].y + 1));
    }
}

fn draw_m3u_confirm_view(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Generate M3U - Confirmation ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref pending) = app.m3u_pending {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  Playlist: {}", pending.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Tracks found: "),
                Span::styled(
                    format!("{}", pending.found),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::raw("  Tracks missing: "),
                Span::styled(
                    format!("{}", pending.missing),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Some tracks are not downloaded yet.",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("Enter", Style::default().fg(Color::Green)),
                Span::raw(" or "),
                Span::styled("y", Style::default().fg(Color::Green)),
                Span::raw(" to generate anyway"),
            ]),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("Esc", Style::default().fg(Color::Red)),
                Span::raw(" or "),
                Span::styled("n", Style::default().fg(Color::Red)),
                Span::raw(" to cancel"),
            ]),
        ];

        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, inner);
    }
}

fn draw_convert_settings_view(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Convert Audio ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Track info
            Constraint::Length(2), // Format row
            Constraint::Length(2), // Quality row
            Constraint::Length(2), // Refresh metadata toggle
            Constraint::Min(0),    // Help text
        ])
        .margin(1)
        .split(inner);

    // Track info
    if let Some(ref pending) = app.convert_pending {
        let track_info = Line::from(vec![
            Span::styled("  Track: ", Style::default().fg(Color::White)),
            Span::styled(&pending.artist, Style::default().fg(Color::Yellow)),
            Span::raw(" - "),
            Span::styled(&pending.title, Style::default().fg(Color::Cyan)),
        ]);
        frame.render_widget(Paragraph::new(track_info), chunks[0]);
    }

    // Format selection
    let mut format_spans = vec![Span::styled(
        "  Format:   ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    for (i, fmt) in FORMAT_OPTIONS.iter().enumerate() {
        let is_selected = i == app.convert_target_format;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        format_spans.push(Span::styled(format!(" {} ", fmt), style));
    }

    let format_line = Paragraph::new(Line::from(format_spans));
    frame.render_widget(format_line, chunks[1]);

    // Quality selection
    let mut quality_spans = vec![Span::styled("  Quality:  ", Style::default().fg(Color::White))];

    for (i, q) in QUALITY_OPTIONS.iter().enumerate() {
        let is_selected = i == app.convert_quality;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        quality_spans.push(Span::styled(format!(" {} ", q), style));
    }

    let quality_line = Paragraph::new(Line::from(quality_spans));
    frame.render_widget(quality_line, chunks[2]);

    // Refresh metadata toggle
    let refresh_status = if app.convert_refresh_metadata {
        Span::styled("[x] Refresh metadata from Spotify", Style::default().fg(Color::Green))
    } else {
        Span::styled("[ ] Refresh metadata from Spotify", Style::default().fg(Color::DarkGray))
    };
    let refresh_line = Paragraph::new(Line::from(vec![Span::raw("  "), refresh_status]));
    frame.render_widget(refresh_line, chunks[3]);

    // Help text
    let help_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  ←/→", Style::default().fg(Color::Yellow)),
            Span::raw("  Change format    "),
            Span::styled("h/l", Style::default().fg(Color::Yellow)),
            Span::raw("  Change quality"),
        ]),
        Line::from(vec![
            Span::styled("  Space", Style::default().fg(Color::Yellow)),
            Span::raw("  Toggle metadata refresh"),
        ]),
        Line::from(vec![
            Span::styled("  Enter", Style::default().fg(Color::Yellow)),
            Span::raw("  Start conversion   "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw("  Cancel"),
        ]),
    ];

    let help = Paragraph::new(help_text).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn draw_convert_confirm_view(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Delete Original File? ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref pending) = app.convert_delete_pending {
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Conversion completed successfully!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Old: "),
                Span::styled(&pending.old_path, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("  New: "),
                Span::styled(&pending.new_path, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Do you want to delete the original file?",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("y", Style::default().fg(Color::Green)),
                Span::raw(" to delete original"),
            ]),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("n", Style::default().fg(Color::Red)),
                Span::raw(" to keep both files"),
            ]),
        ];

        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, inner);
    }
}

fn draw_convert_batch_confirm_view(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Delete Original Files? ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(ref files) = app.convert_batch_delete_pending {
        let count = files.len();
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Batch conversion completed!",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Successfully converted "),
                Span::styled(
                    format!("{}", count),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" file(s)."),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "  Do you want to delete ALL original files?",
                Style::default().fg(Color::White),
            )),
            Line::from(""),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("y", Style::default().fg(Color::Green)),
                Span::raw(" to delete all originals"),
            ]),
            Line::from(vec![
                Span::raw("  Press "),
                Span::styled("n", Style::default().fg(Color::Red)),
                Span::raw(" to keep all files"),
            ]),
        ];

        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, inner);
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let status = Paragraph::new(app.status_message.as_str())
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}
