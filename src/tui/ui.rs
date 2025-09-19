// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use super::app::{App, AppState, LogDisplayMode};
use super::widgets::{centered_rect, create_scrollable_help_widget};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Main layout: Header, Content, (Status), Footer
    let chunks = if app.playback_status.is_some() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Playback status (same height as footer)
                Constraint::Length(3), // Footer
            ])
            .split(size)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer
            ])
            .split(size)
    };

    // Update the visible height based on the content area size
    app.update_visible_height(chunks[1].height as usize);

    // Draw header
    draw_header(frame, app, chunks[0]);

    // Draw main content area
    draw_content(frame, app, chunks[1]);

    // Draw playback status and footer
    if app.playback_status.is_some() {
        draw_playback_status(frame, app, chunks[2]);
        draw_footer(frame, app, chunks[3]);
    } else {
        draw_footer(frame, app, chunks[2]);
    }

    // Draw help overlay if active
    if app.show_help {
        draw_help_overlay(frame, app, size);
    }

    // Draw error overlays (loading overlay removed)
    if let AppState::Error(msg) = &app.state {
        draw_error_overlay(frame, size, msg)
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let base_text = match &app.state {
        AppState::ProviderSelection => "Select Provider".to_string(),
        AppState::MainMenu => "IPTV Player".to_string(),
        AppState::CategorySelection(content_type) => format!("{} - Categories", content_type),
        AppState::StreamSelection(content_type, category) => {
            format!("{} - {}", content_type, category.category_name)
        }
        AppState::SeasonSelection(series) => series.name.clone(),
        AppState::EpisodeSelection(series, season) => {
            format!("{} - Season {}", series.name, season.season_number)
        }
        AppState::VodInfo(_) => "VOD Info".to_string(),
        AppState::Configuration => "Configuration".to_string(),
        AppState::Playing(name) => format!("Playing: {}", name),
        _ => "IPTV Player".to_string(),
    };

    let header_text = if let Some(provider_name) = &app.current_provider_name {
        if matches!(app.state, AppState::ProviderSelection) {
            // Don't show provider name on provider selection screen
            base_text
        } else {
            format!("{} - {}", provider_name, base_text)
        }
    } else {
        base_text
    };

    let header = Paragraph::new(header_text)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        );

    frame.render_widget(header, area);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect) {
    match app.log_display_mode {
        LogDisplayMode::Side => {
            // Split content area into main panel and side panel
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Min(50),    // Main content
                    Constraint::Length(40), // Side panel (logs/info)
                ])
                .split(area);

            // Draw main content list
            draw_main_list(frame, app, chunks[0]);

            // Draw side panel with logs and info
            draw_side_panel(frame, app, chunks[1]);
        }
        LogDisplayMode::None => {
            // Use full width for main content when logs are hidden
            draw_main_list(frame, app, area);
        }
        LogDisplayMode::Full => {
            // Draw logs in full window with scrolling
            draw_full_window_logs(frame, app, area);
        }
    }
}

fn draw_main_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let provider_prefix = if let Some(provider_name) = &app.current_provider_name {
        format!("{} - ", provider_name)
    } else {
        String::new()
    };

    let title = if !app.search_query.is_empty() && !app.search_active {
        format!(
            " {}Content (Filtered: \"{}\") ",
            provider_prefix, app.search_query
        )
    } else if app.search_active {
        format!(
            " {}Content (Search: {}_) ",
            provider_prefix, app.search_query
        )
    } else {
        format!(" {}Content ", provider_prefix)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(title);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Get the items to display based on filter
    let display_indices: Vec<usize> = app.filtered_indices.clone();

    if display_indices.is_empty() {
        let empty_msg = Paragraph::new(if app.search_active || !app.search_query.is_empty() {
            "No items match the search"
        } else {
            "No items to display"
        })
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
        frame.render_widget(empty_msg, inner_area);
        return;
    }

    // Calculate visible range based on filtered items
    let visible_height = inner_area.height as usize;

    // Use content_scroll for VOD info, scroll_offset for others
    let scroll_offset = match &app.state {
        AppState::VodInfo(vod_state) => vod_state.content_scroll,
        _ => app.scroll_offset,
    };

    let start = scroll_offset.min(display_indices.len().saturating_sub(1));
    let end = (start + visible_height).min(display_indices.len());

    // Safety check to prevent panic
    if start > display_indices.len() {
        return;
    }

    // Create list items with selection highlighting
    let items: Vec<ListItem> = display_indices[start..end]
        .iter()
        .map(|&item_idx| {
            let item = &app.items[item_idx];

            // Check if this is a separator (empty string)
            if item.is_empty() {
                // Create a separator line
                let separator = "─".repeat(inner_area.width as usize);
                return ListItem::new(
                    Line::from(separator).style(Style::default().fg(Color::DarkGray)),
                );
            }

            // Check if we're in VOD info mode and determine highlighting behavior
            let should_highlight = match &app.state {
                AppState::VodInfo(_) => {
                    // In VOD info mode, only highlight if this is a menu item and it's selected
                    item_idx == app.selected_index
                        && (item.contains("Play Movie")
                            || item.contains("Copy URL")
                            || item.contains("Back"))
                }
                _ => item_idx == app.selected_index,
            };

            let content = if should_highlight {
                Line::from(vec![Span::raw(" > "), Span::raw(item)]).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Line::from(vec![Span::raw("   "), Span::raw(item)])
            };
            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).style(Style::default().fg(Color::White));

    frame.render_widget(list, inner_area);

    // Draw scrollbar if needed
    if display_indices.len() > visible_height {
        draw_scrollbar(
            frame,
            inner_area,
            scroll_offset,
            display_indices.len(),
            visible_height,
        );
    }
}

fn draw_side_panel(frame: &mut Frame, app: &App, area: Rect) {
    // Just draw the logs panel using the full area
    draw_logs_panel(frame, app, area);
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

    // Show most recent logs at the top, limited by visible area
    let visible_count = inner_area.height as usize;

    // Reverse the logs and take the most recent ones
    let log_lines: Vec<Line> = app
        .logs
        .iter()
        .rev() // Most recent first
        .take(visible_count)
        .map(|(timestamp, msg)| {
            let time_str = timestamp.format("%H:%M:%S").to_string();
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", time_str),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(msg.clone(), Style::default().fg(Color::Gray)),
            ])
        })
        .collect();

    let logs = Paragraph::new(log_lines).wrap(Wrap { trim: true });

    frame.render_widget(logs, inner_area);
}

fn draw_full_window_logs(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Logs (Full View) - Press ESC to return ");

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.logs.is_empty() {
        let empty_msg = Paragraph::new("No logs to display")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(empty_msg, inner_area);
        return;
    }

    // Calculate visible range based on scroll position
    let visible_count = inner_area.height as usize;

    // Create reversed logs with timestamps
    let reversed_logs: Vec<_> = app.logs.iter().rev().collect();
    let total_logs = reversed_logs.len();

    // Adjust scroll offset to keep selected line visible
    if app.log_selected_index >= app.log_scroll_offset + visible_count {
        app.log_scroll_offset = app.log_selected_index.saturating_sub(visible_count - 1);
    }

    let end_idx = (app.log_scroll_offset + visible_count).min(total_logs);

    // Create log items with highlighting for selected line and timestamps
    let log_items: Vec<ListItem> = reversed_logs[app.log_scroll_offset..end_idx]
        .iter()
        .enumerate()
        .map(|(idx, (timestamp, msg))| {
            let actual_idx = app.log_scroll_offset + idx;
            let time_str = timestamp.format("%H:%M:%S").to_string();
            let formatted_msg = format!("[{}] {}", time_str, msg);

            let style = if actual_idx == app.log_selected_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(formatted_msg).style(style)
        })
        .collect();

    let logs_list = List::new(log_items);
    frame.render_widget(logs_list, inner_area);

    // Draw scrollbar indicator if there are more logs than visible
    if total_logs > visible_count {
        let scrollbar_info = format!(" [{}/{}] ", app.log_selected_index + 1, total_logs);
        let scrollbar_area = Rect {
            x: area.x + area.width - scrollbar_info.len() as u16 - 1,
            y: area.y,
            width: scrollbar_info.len() as u16,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(scrollbar_info).style(Style::default().fg(Color::Yellow)),
            scrollbar_area,
        );
    }
}

fn draw_playback_status(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(status) = &app.playback_status {
        // Format time as MM:SS or HH:MM:SS for longer content
        let format_time = |seconds: f64| -> String {
            let total_secs = seconds as u64;
            let hours = total_secs / 3600;
            let mins = (total_secs % 3600) / 60;
            let secs = total_secs % 60;

            if hours > 0 {
                format!("{:02}:{:02}:{:02}", hours, mins, secs)
            } else {
                format!("{:02}:{:02}", mins, secs)
            }
        };

        // Build left side: Playing status and name
        let mut left_parts = vec![];

        if status.is_playing {
            left_parts.push("▶ Playing".to_string());
        } else {
            left_parts.push("⏸ Paused".to_string());
        }

        // Add provider and stream name if available
        if let Some(ref stream_name) = app.current_stream_name {
            // Include provider name if available
            let full_title = if let Some(ref provider) = app.current_provider_name {
                format!("[{}] {}", provider, stream_name)
            } else {
                stream_name.clone()
            };

            // Truncate title if too long
            let max_title_len = 50;
            let display_title = if full_title.len() > max_title_len {
                format!("{}...", &full_title[..max_title_len - 3])
            } else {
                full_title
            };
            left_parts.push(display_title);
        }

        // Build middle: resolution
        let mut middle_parts = vec![];
        if let (Some(width), Some(height)) = (status.width, status.height) {
            middle_parts.push(format!("{}x{}", width, height));
        }

        // Build right side: position/duration and buffer
        let mut right_parts = vec![];

        // Position/Duration
        if status.duration > 0.0 {
            right_parts.push(format!(
                "{} / {}",
                format_time(status.position),
                format_time(status.duration)
            ));
        } else {
            right_parts.push(format_time(status.position));
        }

        // Buffer info
        if status.cache_duration > 0.0 {
            right_parts.push(format!("Buffer: {:.0}s", status.cache_duration));
        }

        // Calculate spacing
        let left_text = left_parts.join(" ");
        let middle_text = middle_parts.join(" ");
        let right_text = right_parts.join(" | ");

        let total_width = area.width as usize;
        let left_len = left_text.len();
        let middle_len = middle_text.len();
        let right_len = right_text.len();

        // Build the complete status line with proper spacing
        let status_text = if total_width > left_len + middle_len + right_len + 4 {
            // We have enough space for everything
            let left_padding = 1;
            let right_padding = 1;
            let available = total_width - left_padding - right_padding;

            // Calculate positions
            let middle_pos = (available - middle_len) / 2;
            let right_pos = available - right_len;

            // Build with spacing
            let mut line = " ".to_string(); // Left padding
            line.push_str(&left_text);

            // Add spaces to position middle text
            if middle_pos > left_len + 2 && !middle_text.is_empty() {
                line.push_str(&" ".repeat(middle_pos - left_len - 1));
                line.push_str(&middle_text);
            }

            // Add spaces to position right text
            if right_pos > line.len() - 1 {
                line.push_str(&" ".repeat(right_pos - line.len() + 1));
                line.push_str(&right_text);
            }

            line
        } else {
            // Not enough space, just concatenate with separators
            format!(" {} | {} | {} ", left_text, middle_text, right_text)
        };

        let status_widget = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Cyan))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            );

        frame.render_widget(status_widget, area);
    }
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let footer_text = if let Some(msg) = &app.status_message {
        msg.clone()
    } else {
        // Special footer for full log view
        if matches!(app.log_display_mode, LogDisplayMode::Full) {
            " ↑↓/jk: Navigate | PgUp/PgDn: Page | Home/End: Jump | Esc: Return | Ctrl+.: Toggle Mode ".to_string()
        } else {
            let log_mode_text = match app.log_display_mode {
                LogDisplayMode::Side => "Logs→Full",
                LogDisplayMode::None => "Show Logs",
                LogDisplayMode::Full => "Hide Logs", // This shouldn't be reached but included for completeness
            };

            match &app.state {
                AppState::VodInfo(_) => {
                    format!(
                        " ↑↓: Menu | PgUp/PgDn/Space/Shift+Space: Scroll | Enter: Select | Esc/b: Back | Ctrl+.: {} | ?: Help ",
                        log_mode_text
                    )
                }
                _ => {
                    format!(
                        " ↑↓/jk: Navigate | Enter: Select | Esc/b: Back | Ctrl+.: {} | ?: Help | q: Quit ",
                        log_mode_text
                    )
                }
            }
        }
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
    for i in scrollbar_chars
        .iter_mut()
        .skip(scrollbar_pos)
        .take(scrollbar_size)
    {
        *i = '█';
    }

    let scrollbar_text: String = scrollbar_chars.into_iter().collect();
    let scrollbar = Paragraph::new(scrollbar_text).style(Style::default().fg(Color::DarkGray));

    let scrollbar_area = Rect {
        x: area.x + area.width - 1,
        y: area.y,
        width: 1,
        height: area.height,
    };

    frame.render_widget(scrollbar, scrollbar_area);
}

fn draw_help_overlay(frame: &mut Frame, app: &App, area: Rect) {
    let help_area = centered_rect(60, 80, area);
    frame.render_widget(Clear, help_area);

    // Calculate visible height for the help text (accounting for borders)
    let visible_height = help_area.height.saturating_sub(2) as usize;

    // Create scrollable help widget
    let help_widget = create_scrollable_help_widget(app.help_scroll_offset, visible_height);
    frame.render_widget(help_widget, help_area);
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
