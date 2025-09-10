// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use reqwest::Client;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

pub struct VlcPlayer {
    http_client: Client,
    port: u16,
    password: String,
    vlc_process: Option<Child>,
}

impl VlcPlayer {
    pub fn new(port: u16, password: String) -> Self {
        Self {
            http_client: Client::new(),
            port,
            password,
            vlc_process: None,
        }
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
            .arg("--quiet") // Reduce console output
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        debug!("Executing VLC command: {:?}", cmd);
        
        let child = cmd
            .spawn()
            .context("Failed to start VLC. Is VLC installed?")?;

        self.vlc_process = Some(child);
        info!("VLC process started, waiting for HTTP interface...");

        // Wait for HTTP interface to be ready
        for i in 0..10 {
            sleep(Duration::from_millis(500)).await;
            if self.is_interface_ready().await {
                info!("VLC HTTP interface ready after {} ms", (i + 1) * 500);
                return Ok(());
            }
            debug!("VLC HTTP interface not ready yet, attempt {}/10", i + 1);
        }

        error!("VLC HTTP interface failed to start after 5 seconds");
        Err(anyhow::anyhow!(
            "VLC HTTP interface failed to start after 5 seconds"
        ))
    }

    /// Check if VLC HTTP interface is responding
    async fn is_interface_ready(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/requests/status.xml", self.port);

        match self
            .http_client
            .get(&url)
            .basic_auth("", Some(&self.password))
            .timeout(Duration::from_secs(1))
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
                debug!("VLC HTTP interface check failed: {}", e);
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
        let _ = self.http_client
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

    /// Stop VLC playback
    pub async fn stop(&mut self) -> Result<()> {
        debug!("Stopping VLC playback");
        
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
        }

        // Kill the process if it exists
        if let Some(mut child) = self.vlc_process.take() {
            debug!("Killing VLC process");
            let _ = child.kill();
            let _ = child.wait();
            info!("VLC process terminated");
        }

        Ok(())
    }

    /// Check if VLC is running
    pub async fn is_running(&mut self) -> bool {
        // Simply check if the HTTP interface is responding
        // The process field might be None but VLC could still be running
        self.is_interface_ready().await
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
        // Clean up VLC process on drop
        if let Some(mut child) = self.vlc_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
