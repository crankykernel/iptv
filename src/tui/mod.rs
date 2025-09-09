// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod app;
pub mod event;
pub mod ui;
pub mod widgets;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

use crate::config::ProviderConfig;
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
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
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
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

pub async fn run_tui(providers: Vec<ProviderConfig>, player: Player) -> Result<()> {
    let mut tui = Tui::new()?;
    tui.init()?;

    let mut app = App::new(providers, player);
    let res = run_app(&mut tui, &mut app).await;

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
            std::time::Duration::from_millis(1000), // Update every second if no events
            tui.event_handler.next(),
        )
        .await;

        let should_redraw = match event {
            Ok(Ok(Event::Key(key_event))) => {
                if let Some(app::Action::Quit) = app.handle_key_event(key_event).await {
                    break;
                }
                true // Always redraw after key events
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
                matches!(app.state, app::AppState::Playing(_)) // Only redraw if playing
            }
        };

        if should_redraw {
            tui.draw(app)?;
        }
    }

    Ok(())
}
