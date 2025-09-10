// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use reqwest::Client;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

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
        // Kill any existing VLC process
        self.stop().await?;

        // Start VLC with HTTP interface
        let mut cmd = Command::new("vlc");
        cmd.arg("--intf")
            .arg("http")
            .arg("--http-host")
            .arg("127.0.0.1")
            .arg("--http-port")
            .arg(self.port.to_string())
            .arg("--http-password")
            .arg(&self.password)
            .arg("--no-video-title-show") // Don't show title on video
            .arg("--no-qt-error-dialogs") // Suppress error dialogs
            .arg("--quiet") // Reduce console output
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        let child = cmd
            .spawn()
            .context("Failed to start VLC. Is VLC installed?")?;

        self.vlc_process = Some(child);

        // Wait for HTTP interface to be ready
        for _ in 0..10 {
            sleep(Duration::from_millis(500)).await;
            if self.is_interface_ready().await {
                return Ok(());
            }
        }

        Err(anyhow::anyhow!(
            "VLC HTTP interface failed to start after 5 seconds"
        ))
    }

    /// Check if VLC HTTP interface is responding
    async fn is_interface_ready(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/requests/status.xml", self.port);
        
        match self.http_client
            .get(&url)
            .basic_auth("", Some(&self.password))
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }

    /// Play or replace current video with new URL
    pub async fn play(&self, video_url: &str) -> Result<()> {
        // First, clear the playlist
        let clear_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=pl_empty",
            self.port
        );
        
        self.http_client
            .get(&clear_url)
            .basic_auth("", Some(&self.password))
            .send()
            .await
            .context("Failed to clear VLC playlist")?;

        // Then add and play the new video
        let play_url = format!(
            "http://127.0.0.1:{}/requests/status.xml?command=in_play&input={}",
            self.port,
            urlencoding::encode(video_url)
        );

        let response = self.http_client
            .get(&play_url)
            .basic_auth("", Some(&self.password))
            .send()
            .await
            .context("Failed to send play command to VLC")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "VLC HTTP interface returned error: {}",
                response.status()
            ));
        }

        Ok(())
    }

    /// Stop VLC playback
    pub async fn stop(&mut self) -> Result<()> {
        // Try to stop via HTTP first
        if self.is_interface_ready().await {
            let stop_url = format!(
                "http://127.0.0.1:{}/requests/status.xml?command=pl_stop",
                self.port
            );
            
            let _ = self.http_client
                .get(&stop_url)
                .basic_auth("", Some(&self.password))
                .send()
                .await;
        }

        // Kill the process if it exists
        if let Some(mut child) = self.vlc_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        Ok(())
    }

    /// Check if VLC is running
    pub async fn is_running(&mut self) -> bool {
        if let Some(child) = self.vlc_process.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    self.vlc_process = None;
                    false
                }
                Ok(None) => {
                    // Still running, check HTTP interface
                    self.is_interface_ready().await
                }
                Err(_) => false,
            }
        } else {
            false
        }
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