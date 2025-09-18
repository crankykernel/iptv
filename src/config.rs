// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: Option<String>, // Optional ID for persistent identification
    pub name: Option<String>,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            providers: vec![ProviderConfig {
                id: None,
                name: Some("Example Provider".to_string()),
                url: "https://your-server.com:port/player_api.php".to_string(),
                username: "your-username".to_string(),
                password: "your-password".to_string(),
            }],
        }
    }
}

impl Config {
    /// Get the default config directory path (~/.config/iptv)
    pub fn default_config_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("iptv"))
    }

    /// Get the default config file path (~/.config/iptv/config.toml)
    pub fn default_config_path() -> Option<PathBuf> {
        Self::default_config_dir().map(|dir| dir.join("config.toml"))
    }

    /// Ensure the config directory exists
    pub fn ensure_config_dir() -> Result<PathBuf> {
        let config_dir = Self::default_config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).with_context(|| {
                format!(
                    "Failed to create config directory: {}",
                    config_dir.display()
                )
            })?;
        }

        Ok(config_dir)
    }
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config =
            toml::from_str(&content).with_context(|| "Failed to parse TOML configuration")?;

        Ok(config)
    }

    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Config {
        Self::load(&path).unwrap_or_else(|_| {
            eprintln!("Warning: Could not load config file, using defaults");
            Self::default()
        })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize config to TOML")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;

        Ok(())
    }
}
