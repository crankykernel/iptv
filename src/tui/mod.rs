// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod app;
pub mod event;
pub mod ui;
pub mod widgets;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;

use crate::player::Player;

pub use app::App;
pub use event::{Event, EventHandler};

pub struct Tui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    pub event_handler: EventHandler,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        let event_handler = EventHandler::new(50); // 50ms tick rate for smooth updates

        Ok(Self {
            terminal,
            event_handler,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        self.terminal.hide_cursor()?;
        self.terminal.clear()?;
        Ok(())
    }

    pub fn draw(&mut self, app: &mut App) -> Result<()> {
        self.terminal.draw(|frame| ui::draw(frame, app))?;
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

pub async fn run_tui(
    config: crate::config::Config,
    player: Player,
    provider: Option<String>,
) -> Result<()> {
    let mut tui = Tui::new()?;
    tui.init()?;

    let mut app = App::new(config, player.clone(), provider).await;
    let res = run_app(&mut tui, &mut app).await;

    // Clean up player resources before exiting
    let _ = player.shutdown().await;

    tui.exit()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app(tui: &mut Tui, app: &mut App) -> Result<()> {
    // Draw once initially
    tui.draw(app)?;

    // Track last playback status update time for smart redraws
    let mut last_status_update = std::time::Instant::now();

    // Track resize events for debouncing
    let mut last_resize = std::time::Instant::now();
    let mut pending_resize = false;
    let resize_debounce_ms = 50; // Wait 50ms after last resize before redrawing

    // Frame rate limiting - prevent drawing more than 60fps (16ms between frames)
    let mut last_draw = std::time::Instant::now();
    let min_frame_time = std::time::Duration::from_millis(16);

    loop {
        // Get next event (now includes periodic ticks)
        let event = tui.event_handler.next().await;

        let should_redraw = match event {
            Ok(Event::Key(key_event)) => {
                match app.handle_key_event(key_event).await {
                    Some(app::Action::Quit) => break,
                    Some(app::Action::CacheRefresh) => {
                        // Exit TUI temporarily to run cache refresh
                        tui.exit()?;

                        // Get provider name before mutable borrow
                        let provider_name = app
                            .current_provider_name
                            .clone()
                            .unwrap_or_else(|| "Unknown".to_string());

                        // Run the same cache refresh as CLI with progress enabled
                        if let Some(api) = &mut app.current_api {
                            // Enable progress bars for the refresh operation
                            api.enable_progress();

                            // This is the exact same call the CLI makes
                            match api.refresh_cache().await {
                                Ok(_) => {
                                    println!("\n✓ Cache refreshed for {}", provider_name);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "\nWarning: Failed to refresh cache for {}: {}",
                                        provider_name, e
                                    );
                                }
                            }

                            // Disable progress bars again for TUI mode
                            api.disable_progress();
                        }

                        // Clear local TUI caches
                        app.clear_internal_caches();

                        // Wait for user to continue
                        println!("\nPress Enter to return to the TUI...");
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;

                        // Re-initialize TUI
                        tui.init()?;
                        true // Redraw after returning
                    }
                    _ => true, // Always redraw after key events
                }
            }
            Ok(Event::Resize(_, _)) => {
                // Debounce resize events - mark as pending but don't redraw immediately
                last_resize = std::time::Instant::now();
                pending_resize = true;
                false // Don't redraw immediately
            }
            Ok(Event::Mouse(_)) => false, // Don't redraw on mouse events we don't handle
            Ok(Event::Tick) => {
                // Periodic update
                app.tick();
                let mut needs_redraw = app.async_tick().await;

                // Check if we have a pending resize that's been stable for debounce period
                if pending_resize
                    && last_resize.elapsed() > std::time::Duration::from_millis(resize_debounce_ms)
                {
                    pending_resize = false;
                    needs_redraw = true; // Force redraw after resize stabilizes
                }

                // Update playback status every 250ms for more responsive display
                if app.playback_status.is_some()
                    && last_status_update.elapsed() > std::time::Duration::from_millis(250)
                {
                    last_status_update = std::time::Instant::now();
                    needs_redraw
                } else {
                    needs_redraw
                }
            }
            Err(e) => return Err(e), // Event handler error
        };

        if should_redraw {
            // Rate limit drawing to prevent excessive updates
            let time_since_last_draw = last_draw.elapsed();
            if time_since_last_draw >= min_frame_time {
                tui.draw(app)?;
                last_draw = std::time::Instant::now();
            }
        }
    }

    Ok(())
}
