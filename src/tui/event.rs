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
    pub fn new(tick_rate: u64) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let sender_clone = sender.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(tick_rate));
            loop {
                let event = if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => Some(Event::Key(key)),
                        Ok(CrosstermEvent::Mouse(mouse)) => Some(Event::Mouse(mouse)),
                        Ok(CrosstermEvent::Resize(width, height)) => {
                            Some(Event::Resize(width, height))
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(event) = event {
                    if sender_clone.send(event).is_err() {
                        break;
                    }
                }

                interval.tick().await;
                if sender_clone.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> Result<Event> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("Event channel closed"))
    }
}