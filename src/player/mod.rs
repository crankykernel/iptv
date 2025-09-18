// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod mpv;

use anyhow::{Context, Result};
use mpv::MpvPlayer;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use tokio::sync::Mutex;
use tracing::{debug, error, warn};

pub struct Player {
    mpv_player: Arc<Mutex<Option<MpvPlayer>>>,
    fallback_process: Arc<Mutex<Option<Child>>>,
    use_mpv: bool,
}

impl Clone for Player {
    fn clone(&self) -> Self {
        Self {
            mpv_player: Arc::new(Mutex::new(None)),
            fallback_process: Arc::new(Mutex::new(None)),
            use_mpv: self.use_mpv,
        }
    }
}

impl Player {
    pub fn new() -> Self {
        let use_mpv = Self::is_mpv_available();

        if use_mpv {
            debug!("MPV detected and will be used as the video player");
        } else {
            debug!(
                "MPV not found! Falling back to basic player mode without remote control support"
            );
        }

        Self {
            mpv_player: Arc::new(Mutex::new(None)),
            fallback_process: Arc::new(Mutex::new(None)),
            use_mpv,
        }
    }

    fn is_mpv_available() -> bool {
        Command::new("mpv")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub fn is_available(&self) -> bool {
        self.use_mpv
    }

    pub async fn play(&self, url: &str) -> Result<()> {
        if !self.use_mpv {
            return Err(anyhow::anyhow!(
                "MPV is not installed. Please install MPV to use this application."
            ));
        }

        // For CLI mode, use the same TUI method for consistent RPC behavior
        self.play_tui(url).await?;
        // Don't use println! as it can corrupt TUI display if called from TUI mode
        debug!("Playing in background...");
        Ok(())
    }

    /// Play video and wait for it to finish (blocking)
    pub async fn play_blocking(&self, url: &str) -> Result<()> {
        if !self.use_mpv {
            return Err(anyhow::anyhow!(
                "MPV is not installed. Please install MPV to use this application."
            ));
        }

        // Launch MPV and wait for it to complete
        let mut cmd = std::process::Command::new("mpv");
        cmd.arg(url)
            .arg("--force-window=yes")
            .arg("--keep-open=yes")
            .arg("--title=IPTV Stream")
            .arg("--geometry=1280x720")
            .arg("--autofit-larger=90%x90%");

        // Run MPV and wait for it to exit
        let status = cmd.status().context("Failed to start MPV")?;

        if !status.success()
            && let Some(code) = status.code()
        {
            // Exit code 4 is normal user quit in MPV
            if code != 4 {
                return Err(anyhow::anyhow!("MPV exited with code: {}", code));
            }
        }

        Ok(())
    }

    /// Play video in completely disassociated window - no RPC, won't be killed/replaced
    pub async fn play_disassociated(&self, url: &str) -> Result<()> {
        if !self.use_mpv {
            return Err(anyhow::anyhow!(
                "MPV is not installed. Please install MPV to use this application."
            ));
        }

        // Launch MPV directly without any IPC/RPC socket
        // Use setsid to ensure it's fully detached
        let mut cmd = if cfg!(target_os = "linux") {
            let mut setsid_cmd = std::process::Command::new("setsid");
            setsid_cmd.arg("mpv");
            setsid_cmd.arg(url);
            setsid_cmd
        } else {
            let mut mpv_cmd = std::process::Command::new("mpv");
            mpv_cmd.arg(url);
            mpv_cmd
        };

        // Add nice defaults for the disassociated window
        cmd.arg("--force-window=yes")
            .arg("--keep-open=yes")
            .arg("--title=IPTV Stream (Independent)")
            .arg("--geometry=1280x720")
            .arg("--autofit-larger=90%x90%")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null());

        cmd.spawn()
            .context("Failed to start MPV in disassociated mode")?;

        Ok(())
    }

    /// Play video in terminal for debugging - shows MPV output with RPC support
    pub async fn play_in_terminal(&self, url: &str) -> Result<()> {
        if !self.use_mpv {
            return Err(anyhow::anyhow!(
                "MPV is not installed. Please install MPV to use this application."
            ));
        }

        // First try to connect to an existing MPV instance
        if let Some(existing_mpv) = MpvPlayer::try_connect_existing().await {
            debug!("Found existing MPV instance via RPC, sending new stream");
            existing_mpv.play(url).await?;
            // Don't use println! as it corrupts the TUI display
            debug!("Sent stream to existing MPV instance via RPC");
            return Ok(());
        }

        // No existing instance, launch MPV in a terminal to see output
        // But with IPC socket enabled for future RPC connections
        let socket_path = std::env::var("XDG_STATE_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").expect("HOME environment variable not set");
                std::path::PathBuf::from(home).join(".local").join("state")
            })
            .join("iptv")
            .join("mpv.sock");

        // Ensure the directory exists
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        // Clean up old socket if it exists
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        // Try different terminal emulators in order of preference
        let terminals = [
            ("alacritty", vec!["-e"]),
            ("kitty", vec![]),
            ("konsole", vec!["-e"]),
            ("gnome-terminal", vec!["--"]),
            ("xfce4-terminal", vec!["-x"]),
            ("mate-terminal", vec!["-x"]),
            ("xterm", vec!["-e"]),
        ];

        let mut terminal_cmd = None;
        for (term, args) in terminals.iter() {
            if Command::new(term)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                terminal_cmd = Some((term.to_string(), args.clone()));
                break;
            }
        }

        let (terminal, term_args) = terminal_cmd.ok_or_else(|| {
            anyhow::anyhow!("No terminal emulator found. Please install one of: alacritty, kitty, konsole, gnome-terminal, xfce4-terminal, mate-terminal, or xterm")
        })?;

        let mut cmd = Command::new(&terminal);

        // Add terminal-specific arguments
        for arg in term_args {
            cmd.arg(arg);
        }

        // Add MPV command with visible output AND IPC socket for RPC
        cmd.arg("mpv")
            .arg(url)
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .arg("--idle=yes") // Keep running for new streams
            .arg("--force-window=yes")
            .arg("--keep-open=yes")
            .arg("--title=IPTV Stream (Terminal)")
            .arg("--geometry=1280x720")
            .arg("--autofit-larger=90%x90%")
            .arg("--osc=yes")
            .arg("--osd-bar=yes")
            .arg("-v") // Verbose output for debugging
            .stdin(Stdio::null());

        cmd.spawn().context(format!(
            "Failed to start {} with MPV for debugging",
            terminal
        ))?;

        // Don't use println! as it corrupts TUI display
        debug!("Started MPV in terminal with RPC support");
        debug!("New streams will be sent to this instance via RPC");

        Ok(())
    }

    /// Play video in detached mode for rofi - starts MPV with RPC then exits
    pub async fn play_detached(&self, url: &str) -> Result<()> {
        if !self.use_mpv {
            return Err(anyhow::anyhow!(
                "MPV is not installed. Please install MPV to use this application."
            ));
        }

        // First try to connect to an existing MPV instance
        if let Some(existing_mpv) = MpvPlayer::try_connect_existing().await {
            debug!("Found existing MPV instance, reusing it");
            existing_mpv.play(url).await?;
            // Don't detach or stop - just let it continue playing
            return Ok(());
        }

        // No existing instance, start a new one
        debug!("No existing MPV instance found, starting new one");
        let mut mpv_guard = self.mpv_player.lock().await;

        // Clean up any old instance
        if let Some(mut old_mpv) = mpv_guard.take() {
            let _ = old_mpv.stop().await;
        }

        let mut mpv = MpvPlayer::new();
        mpv.launch().await?;
        mpv.play(url).await?;

        // Detach the MPV process so it continues running after we exit
        mpv.detach();

        // Drop the mpv instance - it won't kill the process since we detached it
        drop(mpv);

        Ok(())
    }

    /// Play video for TUI mode - runs in background with no terminal output
    pub async fn play_tui(&self, url: &str) -> Result<()> {
        debug!("Playing video in TUI mode");

        if self.use_mpv {
            // Use MPV IPC socket for TUI mode
            let mut mpv_guard = self.mpv_player.lock().await;

            // Check if we need to initialize or restart MPV
            let needs_restart = if let Some(mpv) = mpv_guard.as_mut() {
                let is_running = mpv.is_running().await;
                debug!("MPV is_running check returned: {}", is_running);
                if !is_running {
                    debug!("MPV is not responding, will restart");
                }
                !is_running
            } else {
                debug!("No MPV instance found in guard");
                true
            };

            if needs_restart {
                debug!("Starting new MPV instance");
                if let Some(mut old_mpv) = mpv_guard.take() {
                    debug!("Cleaning up old MPV instance");
                    let _ = old_mpv.stop().await;
                }

                // First try to connect to an existing MPV instance
                if let Some(existing_mpv) = MpvPlayer::try_connect_existing().await {
                    debug!("Found existing MPV instance, reusing it");
                    existing_mpv.play(url).await?;
                    *mpv_guard = Some(existing_mpv);
                } else {
                    let mut mpv = MpvPlayer::new();
                    mpv.launch().await?;
                    mpv.play(url).await?;
                    *mpv_guard = Some(mpv);
                }
            } else if let Some(mpv) = mpv_guard.as_ref() {
                match mpv.play(url).await {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to play video: {}", e);
                        warn!("Attempting to restart MPV after play failure");
                        drop(mpv_guard);
                        let mut mpv_guard = self.mpv_player.lock().await;

                        let mut mpv = MpvPlayer::new();
                        mpv.launch().await?;
                        mpv.play(url).await?;
                        *mpv_guard = Some(mpv);
                    }
                }
            }
        } else {
            // Fallback mode - just try to launch MPV directly without IPC
            // This won't have remote control but at least will play
            warn!("MPV not detected, attempting fallback launch");

            // Stop any existing playback first
            {
                let mut process_guard = self.fallback_process.lock().await;
                if let Some(mut child) = process_guard.take() {
                    let _ = child.kill();
                }
            }

            let url = url.to_string();

            let mut child = tokio::task::spawn_blocking(move || {
                let mut cmd = Command::new("mpv");

                // Try to suppress terminal output
                cmd.arg("--no-terminal");
                cmd.arg("--really-quiet");
                cmd.arg(&url);

                cmd.stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .stdin(Stdio::null());

                cmd.spawn()
            })
            .await
            .with_context(|| "Failed to spawn blocking task")?
            .with_context(|| "Failed to start MPV - is it installed?")?;

            if let Some(stdout) = child.stdout.take() {
                thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    for _ in reader.lines() {
                        // Just consume the output
                    }
                });
            }

            if let Some(stderr) = child.stderr.take() {
                thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for _ in reader.lines() {
                        // Just consume the output
                    }
                });
            }

            {
                let mut process_guard = self.fallback_process.lock().await;
                *process_guard = Some(child);
            }
        }

        Ok(())
    }

    /// Stop TUI playback
    pub async fn stop_tui(&self) -> Result<()> {
        if self.use_mpv {
            let mut mpv_guard = self.mpv_player.lock().await;
            if let Some(mpv) = mpv_guard.as_mut() {
                mpv.stop_with_kill(false).await?;
            }
        } else {
            let mut process_guard = self.fallback_process.lock().await;
            if let Some(mut child) = process_guard.take() {
                let _ = child.kill();
            }
        }
        Ok(())
    }

    /// Check if player is currently running in TUI mode
    /// Returns (is_running, exit_message)
    pub async fn check_player_status(&self) -> (bool, Option<String>) {
        if self.use_mpv {
            let mut mpv_guard = self.mpv_player.lock().await;
            if let Some(mpv) = mpv_guard.as_mut() {
                let is_running = mpv.is_running().await;

                if !is_running && let Some(exit_status) = mpv.get_last_exit_status() {
                    mpv.clear_last_exit_status();

                    let message = if exit_status.success() {
                        "MPV exited normally (status: 0)".to_string()
                    } else if let Some(code) = exit_status.code() {
                        format!("MPV exited with error code: {}", code)
                    } else {
                        "MPV terminated by signal".to_string()
                    };

                    return (false, Some(message));
                }

                (is_running, None)
            } else {
                (false, None)
            }
        } else {
            let mut process_guard = self.fallback_process.lock().await;
            if let Some(child) = process_guard.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        *process_guard = None;

                        let message = if status.success() {
                            "Player exited normally".to_string()
                        } else if let Some(code) = status.code() {
                            format!("Player exited with error code: {}", code)
                        } else {
                            "Player terminated by signal".to_string()
                        };

                        (false, Some(message))
                    }
                    Ok(None) => (true, None),
                    Err(_) => {
                        *process_guard = None;
                        (false, Some("Failed to check player status".to_string()))
                    }
                }
            } else {
                (false, None)
            }
        }
    }

    /// Check if player is currently running in TUI mode
    pub async fn is_playing_tui(&self) -> bool {
        if self.use_mpv {
            let mut mpv_guard = self.mpv_player.lock().await;
            if let Some(mpv) = mpv_guard.as_mut() {
                mpv.is_running().await
            } else {
                false
            }
        } else {
            let mut process_guard = self.fallback_process.lock().await;
            if let Some(child) = process_guard.as_mut() {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        *process_guard = None;
                        false
                    }
                    Ok(None) => true,
                    Err(_) => {
                        *process_guard = None;
                        false
                    }
                }
            } else {
                false
            }
        }
    }

    /// Shutdown the player and clean up all resources
    pub async fn shutdown(&self) -> Result<()> {
        debug!("Shutting down player");

        if self.use_mpv {
            let mut mpv_guard = self.mpv_player.lock().await;
            if let Some(mut mpv) = mpv_guard.take() {
                let _ = mpv.shutdown().await;
            }
        }

        // Also cleanup any CLI background process
        let mut process_guard = self.fallback_process.lock().await;
        if let Some(mut child) = process_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Ok(())
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}
