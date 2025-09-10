// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use rand::Rng;
use reqwest::Client;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct VlcPlayer {
    http_client: Client,
    port: u16,
    password: String,
    vlc_process: Option<Child>,
    last_exit_status: Option<std::process::ExitStatus>,
}

impl VlcPlayer {
    pub fn new(port: u16, password: String) -> Self {
        Self {
            http_client: Client::new(),
            port,
            password,
            vlc_process: None,
            last_exit_status: None,
        }
    }

    /// Create a new VLC player with random port and password
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();

        // Generate random port between 40000-50000 (less likely to conflict)
        let port = rng.gen_range(40000..50000);

        // Generate random password (16 characters, alphanumeric)
        let password: String = (0..16)
            .map(|_| {
                let idx = rng.gen_range(0..62);
                match idx {
                    0..10 => (b'0' + idx as u8) as char,
                    10..36 => (b'a' + (idx - 10) as u8) as char,
                    36..62 => (b'A' + (idx - 36) as u8) as char,
                    _ => unreachable!(),
                }
            })
            .collect();

        info!(
            "Creating VLC player with random port {} and secure password",
            port
        );

        Self::new(port, password)
    }

    /// Start VLC with HTTP interface enabled
    pub async fn launch(&mut self) -> Result<()> {
        debug!("Launching VLC with HTTP interface on port {}", self.port);

        // Check if VLC is already running
        if self.is_interface_ready().await {
            debug!("VLC is already running, skipping launch");
            return Ok(());
        }

        // Only stop if we have an existing process that's not responding
        if self.vlc_process.is_some() {
            self.stop().await?;
        }

        // Start VLC with HTTP interface
        let mut cmd = Command::new("vlc");
        cmd.arg("--intf")
            .arg("http")
            .arg("--extraintf")
            .arg("qt") // Also show the Qt GUI interface
            .arg("--http-host")
            .arg("127.0.0.1")
            .arg("--http-port")
            .arg(self.port.to_string())
            .arg("--http-password")
            .arg(&self.password)
            .arg("--no-video-title-show") // Don't show title on video
            .arg("--no-qt-system-tray") // Disable system tray icon
            .arg("--qt-auto-raise")
            .arg("0") // Never auto-raise window (0=Never, prevents workspace switching)
            .arg("--qt-continue")
            .arg("0") // Never ask to continue playback (0=Never, 1=Ask, 2=Always)
            .arg("--no-qt-video-autoresize") // Don't resize window to video size
            .arg("--verbose")
            .arg("2"); // Set verbose level for debugging

        // Always pipe stdout and stderr so we can consume them
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Log the exact command being executed
        info!("Starting VLC with command: vlc --intf http --extraintf qt --http-host 127.0.0.1 --http-port {} --http-password {} --no-video-title-show --no-qt-system-tray --qt-auto-raise 0 --qt-continue 0 --no-qt-video-autoresize --verbose 2",
              self.port, if self.password.is_empty() { "(empty)" } else { "(set)" });
        debug!("VLC command object: {:?}", cmd);

        let mut child = cmd
            .spawn()
            .context("Failed to start VLC. Is VLC installed?")?;

        // Spawn threads to consume stdout and stderr to prevent blocking
        let debug_file_logging = std::env::var("RUST_LOG")
            .unwrap_or_default()
            .contains("debug");

        // Handle stdout
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                let mut log_file = if debug_file_logging {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("vlc_stdout.log")
                        .ok()
                        .map(|mut f| {
                            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                            let _ = writeln!(f, "\n=== VLC Started at {} ===", timestamp);
                            f
                        })
                } else {
                    None
                };

                for line in reader.lines().map_while(Result::ok) {
                    // Always log to debug output
                    debug!("VLC stdout: {}", line);

                    if let Some(ref mut file) = log_file {
                        let _ = writeln!(file, "{}", line);
                    }
                }
            });
            debug!("VLC stdout consumer thread started");
        }

        // Handle stderr
        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                let mut log_file = if debug_file_logging {
                    OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("vlc_stderr.log")
                        .ok()
                        .map(|mut f| {
                            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                            let _ = writeln!(f, "\n=== VLC Started at {} ===", timestamp);
                            f
                        })
                } else {
                    None
                };

                for line in reader.lines().map_while(Result::ok) {
                    // Always log to debug output, use warn for stderr as it often contains important info
                    if line.contains("error") || line.contains("ERROR") {
                        warn!("VLC stderr: {}", line);
                    } else {
                        debug!("VLC stderr: {}", line);
                    }

                    if let Some(ref mut file) = log_file {
                        let _ = writeln!(file, "{}", line);
                    }
                }
            });
            debug!("VLC stderr consumer thread started");
        }

        self.vlc_process = Some(child);
        info!("VLC process started with PID, waiting for HTTP interface...");

        // Wait for HTTP interface to be ready
        for i in 0..10 {
            sleep(Duration::from_millis(500)).await;

            // Check if process is still running
            if let Some(ref mut proc) = self.vlc_process {
                match proc.try_wait() {
                    Ok(Some(status)) => {
                        error!("VLC process exited unexpectedly with status: {:?}", status);
                        return Err(anyhow::anyhow!(
                            "VLC process exited unexpectedly with status: {:?}. Check debug logs for VLC output.", 
                            status
                        ));
                    }
                    Ok(None) => {
                        // Process is still running, continue checking
                    }
                    Err(e) => {
                        warn!("Failed to check VLC process status: {}", e);
                    }
                }
            }

            if self.is_interface_ready().await {
                info!("VLC HTTP interface ready after {} ms", (i + 1) * 500);
                return Ok(());
            }
            debug!(
                "VLC HTTP interface not ready yet, attempt {}/10, process still running",
                i + 1
            );
        }

        // Final check to see if process is still alive
        if let Some(ref mut proc) = self.vlc_process {
            if let Ok(Some(status)) = proc.try_wait() {
                error!(
                    "VLC process exited during startup with status: {:?}",
                    status
                );
                return Err(anyhow::anyhow!(
                    "VLC process exited during startup with status: {:?}. Check debug logs for VLC stderr output.", 
                    status
                ));
            }
        }

        error!("VLC HTTP interface failed to start after 5 seconds");
        Err(anyhow::anyhow!(
            "VLC HTTP interface failed to start after 5 seconds. VLC process appears to be running but HTTP interface is not responding."
        ))
    }

    /// Check if VLC HTTP interface is responding
    async fn is_interface_ready(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/requests/status.xml", self.port);

        match self
            .http_client
            .get(&url)
            .basic_auth("", Some(&self.password))
            .timeout(Duration::from_secs(2)) // Increased timeout
            .send()
            .await
        {
            Ok(response) => {
                let is_success = response.status().is_success();
                if !is_success {
                    debug!("VLC HTTP interface returned status: {}", response.status());
                }
                is_success
            }
            Err(e) => {
                // Provide more detailed error information
                if e.is_connect() {
                    debug!("VLC HTTP interface check failed - connection error: {}. VLC may not be running or HTTP interface not yet ready on port {}", e, self.port);
                } else if e.is_timeout() {
                    debug!("VLC HTTP interface check failed - timeout after 2 seconds. VLC may be starting up slowly on port {}", self.port);
                } else if e.is_request() {
                    debug!("VLC HTTP interface check failed - request error: {}. Check if VLC is listening on 127.0.0.1:{}", e, self.port);
                } else {
                    debug!(
                        "VLC HTTP interface check failed - unexpected error: {} (port: {})",
                        e, self.port
                    );
                }
                false
            }
        }
    }

    /// Play or replace current video with new URL
    pub async fn play(&self, video_url: &str) -> Result<()> {
        debug!("Playing video: {}", video_url);

        // Check if VLC is still running first
        if !self.is_interface_ready().await {
            warn!("VLC is not running, cannot play video");
            return Err(anyhow::anyhow!(
                "VLC is not running. Please restart the player."
            ));
        }

        // Small delay to prevent overwhelming VLC
        sleep(Duration::from_millis(100)).await;

        // Stop current playback first
        let stop_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=pl_stop",
            self.port
        );

        debug!("Stopping current playback");
        let _ = self
            .http_client
            .get(&stop_url)
            .basic_auth("", Some(&self.password))
            .timeout(Duration::from_secs(2))
            .send()
            .await;

        // Small delay between commands
        sleep(Duration::from_millis(100)).await;

        // Clear the playlist
        let clear_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=pl_empty",
            self.port
        );

        debug!("Clearing playlist");
        let _ = self
            .http_client
            .get(&clear_url)
            .basic_auth("", Some(&self.password))
            .timeout(Duration::from_secs(2))
            .send()
            .await;

        // Small delay before adding new video
        sleep(Duration::from_millis(100)).await;

        // Then add and play the new video
        let play_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=in_play&input={}",
            self.port,
            urlencoding::encode(video_url)
        );

        debug!("Sending play command to VLC: {}", play_url);

        let response = self
            .http_client
            .get(&play_url)
            .basic_auth("", Some(&self.password))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("Failed to send play command to VLC")?;

        if !response.status().is_success() {
            error!("VLC HTTP interface returned error: {}", response.status());
            return Err(anyhow::anyhow!(
                "VLC HTTP interface returned error: {}",
                response.status()
            ));
        }

        info!("Successfully started playing video in VLC");
        Ok(())
    }

    /// Stop VLC playback and optionally kill the process
    pub async fn stop(&mut self) -> Result<()> {
        self.stop_with_kill(true).await
    }

    /// Stop VLC playback with option to keep process running
    pub async fn stop_with_kill(&mut self, kill_process: bool) -> Result<()> {
        debug!("Stopping VLC playback (kill_process: {})", kill_process);

        // Try to stop via HTTP first
        if self.is_interface_ready().await {
            let stop_url = format!(
                "http://127.0.0.1:{}/requests/status.xml?command=pl_stop",
                self.port
            );

            let _ = self
                .http_client
                .get(&stop_url)
                .basic_auth("", Some(&self.password))
                .send()
                .await;

            // Also clear the playlist to ensure nothing is playing
            let clear_url = format!(
                "http://127.0.0.1:{}/requests/status.xml?command=pl_empty",
                self.port
            );

            let _ = self
                .http_client
                .get(&clear_url)
                .basic_auth("", Some(&self.password))
                .send()
                .await;
        }

        // Kill the process if requested and it exists
        if kill_process {
            if let Some(mut child) = self.vlc_process.take() {
                debug!("Killing VLC process");
                let _ = child.kill();
                let _ = child.wait();
                info!("VLC process terminated");
            }
        }

        Ok(())
    }

    /// Force shutdown VLC - always kills the process
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down VLC player");
        self.stop_with_kill(true).await
    }

    /// Check if VLC is running
    pub async fn is_running(&mut self) -> bool {
        // First check if we have a process handle and if it's still running
        if let Some(ref mut proc) = self.vlc_process {
            match proc.try_wait() {
                Ok(Some(status)) => {
                    debug!("VLC process has exited with status: {:?}", status);
                    self.last_exit_status = Some(status);
                    self.vlc_process = None;
                    return false;
                }
                Ok(None) => {
                    debug!("VLC process is still running (PID exists)");
                }
                Err(e) => {
                    warn!("Failed to check VLC process status: {}", e);
                }
            }
        } else {
            debug!("No VLC process handle stored");
        }

        // Check if the HTTP interface is responding
        let is_ready = self.is_interface_ready().await;
        if !is_ready {
            debug!("VLC is_running returning false - HTTP interface not ready (process handle exists: {})", 
                   self.vlc_process.is_some());
        } else {
            debug!("VLC is_running returning true - HTTP interface is ready");
        }
        is_ready
    }

    /// Get the last exit status if VLC has exited
    pub fn get_last_exit_status(&self) -> Option<std::process::ExitStatus> {
        self.last_exit_status
    }

    /// Clear the last exit status (useful after acknowledging the exit)
    pub fn clear_last_exit_status(&mut self) {
        self.last_exit_status = None;
    }

    /// Pause playback
    pub async fn pause(&self) -> Result<()> {
        let pause_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=pl_pause",
            self.port
        );

        self.http_client
            .get(&pause_url)
            .basic_auth("", Some(&self.password))
            .send()
            .await
            .context("Failed to pause VLC")?;

        Ok(())
    }

    /// Set volume (0-256, where 256 is 100%)
    pub async fn set_volume(&self, volume: u16) -> Result<()> {
        let volume = volume.min(256);
        let volume_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=volume&val={}",
            self.port, volume
        );

        self.http_client
            .get(&volume_url)
            .basic_auth("", Some(&self.password))
            .send()
            .await
            .context("Failed to set VLC volume")?;

        Ok(())
    }
}

impl Drop for VlcPlayer {
    fn drop(&mut self) {
        // Always clean up VLC process on drop to ensure proper shutdown
        if let Some(mut child) = self.vlc_process.take() {
            // Check if the process is still running before attempting to kill
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process already exited, nothing to do
                    debug!("VLC process already exited");
                }
                Ok(None) => {
                    // Process is still running, kill it
                    info!("Terminating VLC process on application exit");
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Err(e) => {
                    // Error checking status, attempt cleanup anyway
                    warn!(
                        "Error checking VLC process status: {}, attempting cleanup",
                        e
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}
