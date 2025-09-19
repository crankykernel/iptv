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
        let event_handler = EventHandler::new(250);
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

pub async fn run_tui(config: crate::config::Config, player: Player) -> Result<()> {
    let mut tui = Tui::new()?;
    tui.init()?;

    let mut app = App::new(config, player.clone());
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

    loop {
        // Use timeout to periodically update even without events
        let event = tokio::time::timeout(
            std::time::Duration::from_millis(250), // Update every 250ms for smooth status updates
            tui.event_handler.next(),
        )
        .await;

        let should_redraw = match event {
            Ok(Ok(Event::Key(key_event))) => {
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
            Ok(Ok(Event::Resize(_, _))) => true, // Redraw on resize
            Ok(Ok(Event::Mouse(_))) => false,    // Don't redraw on mouse events we don't handle
            Ok(Ok(Event::Tick)) => {
                app.tick();
                false // Don't redraw on ticks unless something changed
            }
            Ok(Err(e)) => return Err(e), // Event handler error
            Err(_) => {
                // Timeout - periodic update
                app.tick();
                app.async_tick().await // Returns true if redraw is needed
            }
        };

        if should_redraw {
            tui.draw(app)?;
        }
    }

    Ok(())
}
