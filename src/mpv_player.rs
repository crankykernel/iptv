// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, warn};

pub struct MpvPlayer {
    socket_path: PathBuf,
    mpv_process: Option<Child>,
    last_exit_status: Option<std::process::ExitStatus>,
}

impl Default for MpvPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl MpvPlayer {
    pub fn new() -> Self {
        let socket_path = std::env::temp_dir().join(format!("mpv-socket-{}", std::process::id()));
        Self {
            socket_path,
            mpv_process: None,
            last_exit_status: None,
        }
    }

    /// Send a command to MPV via unix socket
    fn send_command(&self, command: Value) -> Result<Value> {
        let mut socket = UnixStream::connect(&self.socket_path).with_context(|| {
            format!("Failed to connect to MPV socket at {:?}", self.socket_path)
        })?;

        let command_str = serde_json::to_string(&command)?;
        debug!("Sending MPV command: {}", command_str);

        socket.write_all(command_str.as_bytes())?;
        socket.write_all(b"\n")?;

        let mut reader = BufReader::new(socket);
        let mut response = String::new();
        reader.read_line(&mut response)?;

        let parsed: Value = serde_json::from_str(&response)
            .with_context(|| format!("Failed to parse MPV response: {}", response))?;

        if let Some(error) = parsed.get("error").and_then(|e| e.as_str())
            && error != "success"
        {
            return Err(anyhow::anyhow!("MPV command failed: {}", error));
        }

        Ok(parsed)
    }

    /// Check if MPV is responding via socket
    async fn is_socket_ready(&self) -> bool {
        if !self.socket_path.exists() {
            return false;
        }

        match UnixStream::connect(&self.socket_path) {
            Ok(mut socket) => {
                // Try a simple get_property command
                let command = json!({
                    "command": ["get_property", "mpv-version"]
                });

                if let Ok(command_str) = serde_json::to_string(&command)
                    && socket.write_all(command_str.as_bytes()).is_ok()
                    && socket.write_all(b"\n").is_ok()
                {
                    return true;
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Launch MPV with IPC socket enabled
    pub async fn launch(&mut self) -> Result<()> {
        debug!("Launching MPV with IPC socket at {:?}", self.socket_path);

        // Check if MPV is already running
        if self.is_socket_ready().await {
            debug!("MPV is already running, skipping launch");
            return Ok(());
        }

        // Stop any existing process
        if self.mpv_process.is_some() {
            self.stop().await?;
        }

        // Clean up old socket if it exists
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }

        // Start MPV with IPC socket
        let mut cmd = Command::new("mpv");
        cmd.arg(format!("--input-ipc-server={}", self.socket_path.display()))
            .arg("--idle=yes") // Keep MPV running even with no file
            .arg("--force-window=yes") // Always show window
            .arg("--keep-open=yes") // Don't close after playback
            .arg("--no-terminal") // No terminal output in TUI mode
            .arg("--really-quiet") // Suppress all console output
            .arg("--osc=yes") // Enable on-screen controller
            .arg("--osd-bar=yes") // Show OSD bar
            .arg("--title=IPTV Player (MPV)")
            .arg("--geometry=1280x720") // Default window size
            .arg("--autofit-larger=90%x90%"); // Max window size

        // Pipe stdout/stderr to consume them
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        debug!("Starting MPV with IPC socket at: {:?}", self.socket_path);
        debug!("MPV command: {:?}", cmd);

        let mut child = cmd
            .spawn()
            .context("Failed to start MPV. Is MPV installed?")?;

        // Spawn threads to consume stdout and stderr
        let debug_logging = std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug");

        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if debug_logging {
                        debug!("MPV stdout: {}", line);
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if debug_logging {
                        if line.contains("error") || line.contains("ERROR") {
                            warn!("MPV stderr: {}", line);
                        } else {
                            debug!("MPV stderr: {}", line);
                        }
                    }
                }
            });
        }

        self.mpv_process = Some(child);
        debug!("MPV process started, waiting for IPC socket...");

        // Wait for IPC socket to be ready
        for i in 0..20 {
            // Wait up to 10 seconds
            sleep(Duration::from_millis(500)).await;

            // Check if process is still running
            if let Some(ref mut proc) = self.mpv_process {
                match proc.try_wait() {
                    Ok(Some(status)) => {
                        error!("MPV process exited unexpectedly with status: {:?}", status);
                        return Err(anyhow::anyhow!(
                            "MPV process exited unexpectedly with status: {:?}",
                            status
                        ));
                    }
                    Ok(None) => {
                        // Process is still running
                    }
                    Err(e) => {
                        warn!("Failed to check MPV process status: {}", e);
                    }
                }
            }

            if self.is_socket_ready().await {
                debug!("MPV IPC socket ready after {} ms", (i + 1) * 500);
                return Ok(());
            }
            debug!("MPV IPC socket not ready yet, attempt {}/20", i + 1);
        }

        error!("MPV IPC socket failed to start after 10 seconds");
        Err(anyhow::anyhow!(
            "MPV IPC socket failed to start after 10 seconds"
        ))
    }

    /// Play or replace current video with new URL
    pub async fn play(&self, video_url: &str) -> Result<()> {
        debug!("Playing video: {}", video_url);

        // Check if MPV is still running
        if !self.is_socket_ready().await {
            warn!("MPV is not running, cannot play video");
            return Err(anyhow::anyhow!(
                "MPV is not running. Please restart the player."
            ));
        }

        // Stop current playback first
        let _ = self.send_command(json!({
            "command": ["stop"]
        }));

        sleep(Duration::from_millis(100)).await;

        // Load and play the new video
        let command = json!({
            "command": ["loadfile", video_url, "replace"]
        });

        self.send_command(command)
            .context("Failed to send play command to MPV")?;

        debug!("Successfully started playing video in MPV");
        Ok(())
    }

    /// Stop MPV playback and optionally kill the process
    pub async fn stop(&mut self) -> Result<()> {
        self.stop_with_kill(true).await
    }

    /// Stop MPV playback with option to keep process running
    pub async fn stop_with_kill(&mut self, kill_process: bool) -> Result<()> {
        debug!("Stopping MPV playback (kill_process: {})", kill_process);

        // Try to stop via IPC first
        if self.is_socket_ready().await {
            let _ = self.send_command(json!({
                "command": ["stop"]
            }));

            // Clear playlist
            let _ = self.send_command(json!({
                "command": ["playlist-clear"]
            }));
        }

        // Kill the process if requested
        if kill_process {
            if let Some(mut child) = self.mpv_process.take() {
                debug!("Killing MPV process");
                let _ = child.kill();
                let _ = child.wait();
                debug!("MPV process terminated");
            }

            // Clean up socket file
            if self.socket_path.exists() {
                let _ = fs::remove_file(&self.socket_path);
            }
        }

        Ok(())
    }

    /// Force shutdown MPV
    pub async fn shutdown(&mut self) -> Result<()> {
        debug!("Shutting down MPV player");
        self.stop_with_kill(true).await
    }

    /// Detach MPV process - let it continue running independently
    pub fn detach(&mut self) {
        debug!("Detaching MPV process - will continue running independently");
        // Take ownership of the process handle without killing it
        self.mpv_process.take();
        // The socket file will remain for potential reconnection
    }

    /// Check if MPV is running
    pub async fn is_running(&mut self) -> bool {
        // First check if we have a process handle and if it's still running
        if let Some(ref mut proc) = self.mpv_process {
            match proc.try_wait() {
                Ok(Some(status)) => {
                    debug!("MPV process has exited with status: {:?}", status);
                    self.last_exit_status = Some(status);
                    self.mpv_process = None;

                    // Clean up socket file
                    if self.socket_path.exists() {
                        let _ = fs::remove_file(&self.socket_path);
                    }

                    return false;
                }
                Ok(None) => {
                    debug!("MPV process is still running (PID exists)");
                }
                Err(e) => {
                    warn!("Failed to check MPV process status: {}", e);
                }
            }
        } else {
            debug!("No MPV process handle stored");
        }

        // Check if the IPC socket is responding
        let is_ready = self.is_socket_ready().await;
        if !is_ready {
            debug!("MPV is_running returning false - IPC socket not ready");
        } else {
            debug!("MPV is_running returning true - IPC socket is ready");
        }
        is_ready
    }

    /// Get the last exit status if MPV has exited
    pub fn get_last_exit_status(&self) -> Option<std::process::ExitStatus> {
        self.last_exit_status
    }

    /// Clear the last exit status
    pub fn clear_last_exit_status(&mut self) {
        self.last_exit_status = None;
    }

    /// Pause/unpause playback
    pub async fn pause(&self) -> Result<()> {
        self.send_command(json!({
            "command": ["cycle", "pause"]
        }))
        .context("Failed to pause MPV")?;
        Ok(())
    }

    /// Set volume (0-100)
    pub async fn set_volume(&self, volume: u8) -> Result<()> {
        let volume = volume.min(100);
        self.send_command(json!({
            "command": ["set_property", "volume", volume]
        }))
        .context("Failed to set MPV volume")?;
        Ok(())
    }

    /// Seek to position in seconds
    pub async fn seek(&self, position: f64) -> Result<()> {
        self.send_command(json!({
            "command": ["seek", position, "absolute"]
        }))
        .context("Failed to seek in MPV")?;
        Ok(())
    }

    /// Get current playback position
    pub async fn get_position(&self) -> Result<f64> {
        let response = self.send_command(json!({
            "command": ["get_property", "time-pos"]
        }))?;

        response
            .get("data")
            .and_then(|d| d.as_f64())
            .ok_or_else(|| anyhow::anyhow!("Failed to get playback position"))
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        // Clean up MPV process on drop
        if let Some(mut child) = self.mpv_process.take() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    debug!("MPV process already exited");
                }
                Ok(None) => {
                    debug!("Terminating MPV process on cleanup");
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(e) => {
                    warn!(
                        "Error checking MPV process status: {}, attempting cleanup",
                        e
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }

        // Clean up socket file
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}
