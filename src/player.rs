// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::config::PlayerConfig;
use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub struct Player {
    config: PlayerConfig,
}

impl Player {
    pub fn new(config: PlayerConfig) -> Self {
        Self { config }
    }

    pub fn play(&self, url: &str) -> Result<()> {
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
                "Player process failed with exit code: {}", status
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
            format!("Failed to start player in background: {}", self.config.command)
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
}
