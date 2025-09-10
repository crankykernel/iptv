// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
}

pub struct EventHandler {
    #[allow(dead_code)]
    sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new(_tick_rate: u64) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let sender_clone = sender.clone();

        // Use a separate thread for blocking I/O to avoid async overhead
        std::thread::spawn(move || {
            loop {
                // Use a very short timeout (1ms) for responsive input
                // This is still non-blocking but much more responsive
                if event::poll(Duration::from_millis(1)).unwrap_or(false) {
                    let event = match event::read() {
                        Ok(CrosstermEvent::Key(key)) => Some(Event::Key(key)),
                        Ok(CrosstermEvent::Mouse(mouse)) => Some(Event::Mouse(mouse)),
                        Ok(CrosstermEvent::Resize(width, height)) => {
                            Some(Event::Resize(width, height))
                        }
                        _ => None,
                    };

                    if let Some(event) = event
                        && sender_clone.send(event).is_err()
                    {
                        break;
                    }
                } else {
                    // Small sleep to prevent CPU spinning when no events
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        });

        Self {
            #[allow(dead_code)]
            sender,
            receiver,
        }
    }

    pub async fn next(&mut self) -> Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }
}
