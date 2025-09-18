// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn get_help_lines() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "IPTV Player TUI - Help",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/k       - Move up"),
        Line::from("  ↓/j       - Move down"),
        Line::from("  PgUp      - Page up (10 items)"),
        Line::from("  PgDn      - Page down (10 items)"),
        Line::from("  Home      - Jump to first"),
        Line::from("  End       - Jump to last"),
        Line::from("  Enter     - Select item / Play stream"),
        Line::from("  a         - Advanced play menu (for live streams)"),
        Line::from("  Esc/b     - Go back"),
        Line::from("  q         - Quit application"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Special Keys:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  /         - Fuzzy search/filter list"),
        Line::from("  f         - Toggle favourite (in stream/favourite lists)"),
        Line::from("  i         - Toggle ignore (category/channel)"),
        Line::from("  s         - Stop any active playback"),
        Line::from("  ?/F1      - Toggle this help"),
        Line::from("  Ctrl+C    - Force quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "VOD Info Mode:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓         - Navigate menu options"),
        Line::from("  PgUp/PgDn   - Scroll content by page"),
        Line::from("  Space       - Scroll content down by page"),
        Line::from("  Shift+Space - Scroll content up by page"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Features:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  • Browse Live TV, Movies, and TV Series"),
        Line::from("  • Manage favourites with quick access"),
        Line::from("  • Cache management for faster loading"),
        Line::from("  • Multi-provider support"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Help Navigation:",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  ↑/↓       - Scroll help text"),
        Line::from("  PgUp/PgDn - Scroll by page"),
        Line::from("  Esc/?/F1  - Close help"),
        Line::from(""),
        Line::from("Press Esc, ? or F1 to close this help"),
    ]
}

pub fn create_help_widget() -> Paragraph<'static> {
    Paragraph::new(get_help_lines())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(" Help "),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
}

pub fn create_scrollable_help_widget(
    scroll_offset: usize,
    visible_height: usize,
) -> Paragraph<'static> {
    let all_lines = get_help_lines();
    let total_lines = all_lines.len();

    // Calculate the effective scroll offset
    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll = scroll_offset.min(max_scroll);

    // Get the visible lines
    let end_idx = (scroll + visible_height).min(total_lines);
    let visible_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(scroll)
        .take(end_idx - scroll)
        .collect();

    // Create title with scroll indicator if needed
    let title = if max_scroll > 0 {
        format!(" Help (↑↓ to scroll, {}/{}) ", scroll + 1, max_scroll + 1)
    } else {
        " Help ".to_string()
    };

    Paragraph::new(visible_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title(title),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: false })
}
