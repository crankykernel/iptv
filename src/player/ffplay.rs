// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};
use tracing::debug;

#[derive(Default)]
pub struct FfplayPlayer {
    process: Option<Child>,
}

impl FfplayPlayer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_available() -> bool {
        Command::new("ffplay")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    /// Play video in ffplay window
    pub fn play(&mut self, url: &str) -> Result<()> {
        // Kill any existing process
        if let Some(mut proc) = self.process.take() {
            let _ = proc.kill();
            let _ = proc.wait();
        }

        debug!("Starting ffplay with URL: {}", url);

        let mut cmd = Command::new("ffplay");
        cmd.arg(url)
            .arg("-window_title")
            .arg("IPTV Player (ffplay)")
            .arg("-x")
            .arg("1280")
            .arg("-y")
            .arg("720")
            .arg("-autoexit") // Exit when playback ends
            .arg("-infbuf") // Reduce buffering for live streams
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        let child = cmd.spawn().context("Failed to start ffplay")?;

        self.process = Some(child);
        debug!("ffplay started successfully");

        Ok(())
    }

    /// Play video in terminal with visible output for debugging
    pub fn play_in_terminal(&mut self, url: &str) -> Result<()> {
        // Kill any existing process
        if let Some(mut proc) = self.process.take() {
            let _ = proc.kill();
            let _ = proc.wait();
        }

        debug!("Starting ffplay in terminal with URL: {}", url);

        // Try different terminal emulators in order of preference
        let terminals = [
            ("alacritty", vec!["-e"]),
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
            anyhow::anyhow!("No terminal emulator found. Please install one of: alacritty, konsole, gnome-terminal, xfce4-terminal, mate-terminal, or xterm")
        })?;

        let mut cmd = Command::new(&terminal);

        // Add terminal-specific arguments
        for arg in term_args {
            cmd.arg(arg);
        }

        // Add ffplay command with visible output
        cmd.arg("ffplay")
            .arg(url)
            .arg("-window_title")
            .arg("IPTV Player (ffplay Terminal)")
            .arg("-x")
            .arg("1280")
            .arg("-y")
            .arg("720")
            .arg("-autoexit")
            .arg("-infbuf")
            .arg("-stats") // Show statistics
            .stdin(Stdio::null());

        let child = cmd
            .spawn()
            .context(format!("Failed to start {} with ffplay", terminal))?;

        self.process = Some(child);
        debug!("ffplay started in terminal successfully");

        Ok(())
    }

    /// Play video in detached window
    pub fn play_detached(&self, url: &str) -> Result<()> {
        debug!("Starting ffplay in detached mode with URL: {}", url);

        // Use setsid to detach from parent process group on Linux
        let mut cmd = if cfg!(target_os = "linux") {
            let mut setsid_cmd = Command::new("setsid");
            setsid_cmd.arg("ffplay");
            setsid_cmd.arg(url);
            setsid_cmd
        } else {
            let mut ffplay_cmd = Command::new("ffplay");
            ffplay_cmd.arg(url);
            ffplay_cmd
        };

        cmd.arg("-window_title")
            .arg("IPTV Player (ffplay Detached)")
            .arg("-x")
            .arg("1280")
            .arg("-y")
            .arg("720")
            .arg("-autoexit")
            .arg("-infbuf")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        cmd.spawn()
            .context("Failed to start ffplay in detached mode")?;

        debug!("ffplay started in detached mode successfully");
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(mut proc) = self.process.take() {
            debug!("Stopping ffplay process");
            let _ = proc.kill();
            let _ = proc.wait();
        }
    }
}

impl Drop for FfplayPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
