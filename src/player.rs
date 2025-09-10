// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::config::PlayerConfig;
use crate::vlc_player::VlcPlayer;
use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Player {
    config: PlayerConfig,
    current_process: Arc<Mutex<Option<Child>>>,
    vlc_player: Arc<Mutex<Option<VlcPlayer>>>,
}

impl Clone for Player {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            current_process: Arc::new(Mutex::new(None)),
            vlc_player: Arc::new(Mutex::new(None)),
        }
    }
}

impl Player {
    pub fn new(config: PlayerConfig) -> Self {
        Self {
            config,
            current_process: Arc::new(Mutex::new(None)),
            vlc_player: Arc::new(Mutex::new(None)),
        }
    }

    fn is_vlc(&self) -> bool {
        self.config.command.to_lowercase() == "vlc"
    }

    pub fn play(&self, url: &str) -> Result<()> {
        // For VLC in CLI mode, use regular process spawning (not HTTP)
        // This allows VLC to run in foreground with full UI
        let mut cmd = Command::new(&self.config.command);

        // Add configured arguments
        for arg in &self.config.args {
            cmd.arg(arg);
        }

        // Add the URL
        cmd.arg(url);

        println!("Starting player: {} {}", self.config.command, url);
        println!("Press Ctrl+C or quit the player to return to the menu");

        // Run the process in the foreground and wait for it to complete
        let status = cmd.status().with_context(|| {
            format!("Failed to execute player command: {}", self.config.command)
        })?;

        if !status.success() {
            eprintln!("Player exited with error code: {}", status);
            return Err(anyhow::anyhow!(
                "Player process failed with exit code: {}",
                status
            ));
        }

        println!("Player exited successfully");
        Ok(())
    }

    pub fn play_background(&self, url: &str) -> Result<()> {
        let mut cmd = Command::new(&self.config.command);

        // Add configured arguments
        for arg in &self.config.args {
            cmd.arg(arg);
        }

        // Add the URL
        cmd.arg(url);

        // Detach from terminal - redirect stdout/stderr to null for true background execution
        cmd.stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        // Start the process in background and detach
        cmd.spawn().with_context(|| {
            format!(
                "Failed to start player in background: {}",
                self.config.command
            )
        })?;

        Ok(())
    }

    pub fn is_available(&self) -> bool {
        Command::new(&self.config.command)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Play video for TUI mode - runs in background with no terminal output
    pub async fn play_tui(&self, url: &str) -> Result<()> {
        if self.is_vlc() {
            // Use VLC HTTP interface for TUI mode
            let mut vlc_guard = self.vlc_player.lock().await;

            // Initialize VLC player if needed
            if vlc_guard.is_none() {
                let mut vlc = VlcPlayer::new(
                    self.config.vlc.http_port,
                    self.config.vlc.http_password.clone(),
                );
                vlc.launch().await?;
                *vlc_guard = Some(vlc);
            }

            // Play the video using HTTP interface
            if let Some(vlc) = vlc_guard.as_ref() {
                vlc.play(url).await?;
            }
        } else {
            // Use regular process-based approach for non-VLC players
            // Stop any existing playback first (but don't wait for it)
            {
                let mut process_guard = self.current_process.lock().await;
                if let Some(mut child) = process_guard.take() {
                    let _ = child.kill();
                    // Don't wait - let it terminate in background
                }
            }

            // Clone values needed in the closure
            let player_cmd = self.config.command.clone();
            let player_args = self.config.args.clone();
            let url = url.to_string();

            // Spawn the process in a completely detached way
            let child = tokio::task::spawn_blocking(move || {
                let mut cmd = Command::new(&player_cmd);

                // Add configured arguments
                for arg in &player_args {
                    cmd.arg(arg);
                }

                // Add arguments to suppress terminal output and run in background
                // These work for mpv - might need adjustment for other players
                cmd.arg("--no-terminal");
                cmd.arg("--really-quiet"); // Suppress all console output
                cmd.arg("--force-window=immediate"); // Show window immediately
                cmd.arg("--keep-open=no");

                // Add the URL
                cmd.arg(&url);

                // Redirect all output to null to prevent terminal interference
                cmd.stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null());

                cmd.spawn()
            })
            .await
            .with_context(|| "Failed to spawn blocking task")?
            .with_context(|| format!("Failed to start player: {}", self.config.command))?;

            // Store the process handle - minimize lock time
            {
                let mut process_guard = self.current_process.lock().await;
                *process_guard = Some(child);
            }
        }

        Ok(())
    }

    /// Stop TUI playback
    pub async fn stop_tui(&self) -> Result<()> {
        if self.is_vlc() {
            // Stop VLC via HTTP interface
            let mut vlc_guard = self.vlc_player.lock().await;
            if let Some(mut vlc) = vlc_guard.take() {
                vlc.stop().await?;
            }
        } else {
            // Stop regular process
            let mut process_guard = self.current_process.lock().await;
            if let Some(mut child) = process_guard.take() {
                // Try to kill the process
                let _ = child.kill();
                // Don't wait - just let it terminate in the background
                // child.wait() would block the TUI
            }
        }
        Ok(())
    }

    /// Check if player is currently running in TUI mode
    pub async fn is_playing_tui(&self) -> bool {
        if self.is_vlc() {
            // Check VLC status
            let mut vlc_guard = self.vlc_player.lock().await;
            if let Some(vlc) = vlc_guard.as_mut() {
                vlc.is_running().await
            } else {
                false
            }
        } else {
            // Check regular process status
            let mut process_guard = self.current_process.lock().await;
            if let Some(child) = process_guard.as_mut() {
                // Check if process is still running
                match child.try_wait() {
                    Ok(Some(_)) => {
                        // Process has exited
                        *process_guard = None;
                        false
                    }
                    Ok(None) => {
                        // Still running
                        true
                    }
                    Err(_) => {
                        // Error checking status
                        *process_guard = None;
                        false
                    }
                }
            } else {
                false
            }
        }
    }
}
