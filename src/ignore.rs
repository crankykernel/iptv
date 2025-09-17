use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub categories: HashSet<String>,
    #[serde(default)]
    pub channels: HashSet<String>,
}

impl IgnoreConfig {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn toggle_category(&mut self, category: &str) -> Result<bool> {
        let is_ignored = if self.categories.contains(category) {
            self.categories.remove(category);
            false
        } else {
            self.categories.insert(category.to_string());
            true
        };

        self.save()?;
        Ok(is_ignored)
    }

    pub fn toggle_channel(&mut self, channel: &str) -> Result<bool> {
        let is_ignored = if self.channels.contains(channel) {
            self.channels.remove(channel);
            false
        } else {
            self.channels.insert(channel.to_string());
            true
        };

        self.save()?;
        Ok(is_ignored)
    }

    pub fn is_category_ignored(&self, category: &str) -> bool {
        self.categories.contains(category)
    }

    pub fn is_channel_ignored(&self, channel: &str) -> bool {
        self.channels.contains(channel)
    }

    pub fn get_ignored_categories(&self) -> &HashSet<String> {
        &self.categories
    }

    pub fn get_ignored_channels(&self) -> &HashSet<String> {
        &self.channels
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("iptv").join("ignore.toml"))
    }
}
