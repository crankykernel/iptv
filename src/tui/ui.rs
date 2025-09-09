// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, AppState};
use super::widgets::{centered_rect, create_help_widget};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Main layout: Header, Content, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Content
            Constraint::Length(3),  // Footer
        ])
        .split(size);

    // Draw header
    draw_header(frame, app, chunks[0]);

    // Draw main content area
    draw_content(frame, app, chunks[1]);

    // Draw footer
    draw_footer(frame, app, chunks[2]);

    // Draw help overlay if active
    if app.show_help {
        draw_help_overlay(frame, size);
    }

    // Draw loading or error overlays
    match &app.state {
        AppState::Loading(msg) => draw_loading_overlay(frame, size, msg),
        AppState::Error(msg) => draw_error_overlay(frame, size, msg),
        _ => {}
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let header_text = match &app.state {
        AppState::ProviderSelection => "Select Provider",
        AppState::MainMenu => "IPTV Player",
        AppState::CategorySelection(content_type) => {
            &format!("{} - Categories", content_type)
        }
        AppState::StreamSelection(content_type, category) => {
            &format!("{} - {}", content_type, category.category_name)
        }
        AppState::SeasonSelection(series) => &series.name,
        AppState::EpisodeSelection(series, season) => {
            &format!("{} - Season {}", series.name, season.season_number)
        }
        AppState::FavouriteSelection => "Favourites",
        AppState::Playing(name) => &format!("Playing: {}", name),
        _ => "IPTV Player",
    };

    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );

    frame.render_widget(header, area);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    // Split content area into main panel and side panel
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(50),      // Main content
            Constraint::Length(40),   // Side panel (logs/info)
        ])
        .split(area);

    // Draw main content list
    draw_main_list(frame, app, chunks[0]);

    // Draw side panel with logs and info
    draw_side_panel(frame, app, chunks[1]);
}

fn draw_main_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(" Content ");

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.items.is_empty() {
        let empty_msg = Paragraph::new("No items to display")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner_area);
        return;
    }

    // Calculate visible range
    let visible_height = inner_area.height as usize;
    let start = app.scroll_offset;
    let end = (start + visible_height).min(app.items.len());

    // Create list items with selection highlighting
    let items: Vec<ListItem> = app.items[start..end]
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let index = start + i;
            let content = if index == app.selected_index {
                Line::from(vec![
                    Span::raw(" ▶ "),
                    Span::raw(item),
                ])
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Line::from(vec![
                    Span::raw("   "),
                    Span::raw(item),
                ])
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .style(Style::default().fg(Color::White));

    frame.render_widget(list, inner_area);

    // Draw scrollbar if needed
    if app.items.len() > visible_height {
        draw_scrollbar(frame, inner_area, app.scroll_offset, app.items.len(), visible_height);
    }
}

fn draw_side_panel(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),     // Logs
            Constraint::Length(5),   // Progress (if any)
        ])
        .split(area);

    // Draw logs panel
    draw_logs_panel(frame, app, chunks[0]);

    // Draw progress panel if there's progress to show
    if let Some((progress, label)) = &app.progress {
        draw_progress_panel(frame, chunks[1], *progress, label);
    } else {
        draw_info_panel(frame, app, chunks[1]);
    }
}

fn draw_logs_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Logs ");

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.logs.is_empty() {
        return;
    }

    // Show most recent logs that fit in the area
    let visible_count = inner_area.height as usize;
    let start = app.logs.len().saturating_sub(visible_count);
    
    let log_lines: Vec<Line> = app.logs[start..]
        .iter()
        .map(|(_, msg)| {
            Line::from(msg.clone())
                .style(Style::default().fg(Color::Gray))
        })
        .collect();

    let logs = Paragraph::new(log_lines)
        .wrap(Wrap { trim: true });

    frame.render_widget(logs, inner_area);
}

fn draw_progress_panel(frame: &mut Frame, area: Rect, progress: f64, label: &str) {
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Progress "),
        )
        .gauge_style(Style::default().fg(Color::Green))
        .percent((progress * 100.0) as u16)
        .label(label);

    frame.render_widget(gauge, area);
}

fn draw_info_panel(frame: &mut Frame, app: &App, area: Rect) {
    let info_text = match &app.state {
        AppState::StreamSelection(_, _) | AppState::FavouriteSelection => {
            vec![
                Line::from(""),
                Line::from("Press 'f' to toggle favourite"),
                Line::from("Press '?' for help"),
            ]
        }
        AppState::Playing(_) => {
            vec![
                Line::from(""),
                Line::from("Press 's' or ESC to stop"),
                Line::from(""),
            ]
        }
        _ => {
            vec![
                Line::from(""),
                Line::from("Press '?' for help"),
                Line::from("Press 'q' to quit"),
            ]
        }
    };

    let info = Paragraph::new(info_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Info "),
        )
        .style(Style::default().fg(Color::Cyan));

    frame.render_widget(info, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let footer_text = if let Some(msg) = &app.status_message {
        msg.clone()
    } else {
        format!(" Item {} of {} | ↑↓/jk: Navigate | Enter: Select | Esc/b: Back | q: Quit ",
            app.selected_index + 1,
            app.items.len().max(1)
        )
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    frame.render_widget(footer, area);
}

fn draw_scrollbar(frame: &mut Frame, area: Rect, offset: usize, total: usize, visible: usize) {
    if total <= visible {
        return;
    }

    let scrollbar_height = area.height as usize;
    let scrollbar_pos = (offset * scrollbar_height) / total;
    let scrollbar_size = (visible * scrollbar_height) / total;

    let mut scrollbar_chars = vec!['│'; scrollbar_height];
    for i in scrollbar_chars.iter_mut().skip(scrollbar_pos).take(scrollbar_size) {
        *i = '█';
    }

    let scrollbar_text: String = scrollbar_chars.into_iter().collect();
    let scrollbar = Paragraph::new(scrollbar_text)
        .style(Style::default().fg(Color::DarkGray));

    let scrollbar_area = Rect {
        x: area.x + area.width - 1,
        y: area.y,
        width: 1,
        height: area.height,
    };

    frame.render_widget(scrollbar, scrollbar_area);
}

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let help_area = centered_rect(60, 80, area);
    frame.render_widget(Clear, help_area);
    frame.render_widget(create_help_widget(), help_area);
}

fn draw_loading_overlay(frame: &mut Frame, area: Rect, message: &str) {
    let loading_area = centered_rect(40, 20, area);
    frame.render_widget(Clear, loading_area);

    let loading = Paragraph::new(vec![
        Line::from(""),
        Line::from("⏳ Loading...").style(Style::default().fg(Color::Yellow)),
        Line::from(""),
        Line::from(message).style(Style::default().fg(Color::Gray)),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Please Wait "),
    )
    .alignment(Alignment::Center);

    frame.render_widget(loading, loading_area);
}

fn draw_error_overlay(frame: &mut Frame, area: Rect, message: &str) {
    let error_area = centered_rect(50, 30, area);
    frame.render_widget(Clear, error_area);

    let error = Paragraph::new(vec![
        Line::from(""),
        Line::from("❌ Error").style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from(message).style(Style::default().fg(Color::White)),
        Line::from(""),
        Line::from("Press Enter or Esc to continue").style(Style::default().fg(Color::Gray)),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(" Error "),
    )
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });

    frame.render_widget(error, error_area);
}