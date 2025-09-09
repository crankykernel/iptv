// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub providers: Vec<ProviderConfig>,
    pub player: PlayerConfig,
    pub cache: CacheConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: Option<String>,
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub page_size: usize,
    pub search_debounce_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            providers: vec![ProviderConfig {
                name: Some("Example Provider".to_string()),
                url: "https://your-server.com:port/player_api.php".to_string(),
                username: "your-username".to_string(),
                password: "your-password".to_string(),
            }],
            player: PlayerConfig {
                command: "mpv".to_string(),
                args: vec!["--fs".to_string(), "--quiet".to_string()],
            },
            cache: CacheConfig {
                max_entries: 1000,
            },
            ui: UiConfig {
                page_size: 20,
                search_debounce_ms: 300,
            },
        }
    }
}

impl Config {
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
