// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use crate::config::PlayerConfig;

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
        
        // Redirect stdout and stderr to avoid output pollution
        cmd.stdout(Stdio::null())
           .stderr(Stdio::null());

        println!("Starting player: {} {}", self.config.command, url);
        
        let status = cmd.status()
            .with_context(|| format!("Failed to execute player command: {}", self.config.command))?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Player exited with non-zero status: {:?}",
                status.code()
            ));
        }

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