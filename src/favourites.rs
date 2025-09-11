// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::config::Config;
use crate::xtream::FavouriteStream;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavouritesData {
    pub favourites: Vec<FavouriteStream>,
}

/// Manages favorites stored in the config directory (not cache)
#[derive(Debug)]
pub struct FavouritesManager {
    favourites_dir: PathBuf,
}

impl FavouritesManager {
    pub fn new() -> Result<Self> {
        let config_dir = Config::ensure_config_dir()?;
        let favourites_dir = config_dir.join("favourites");

        // Ensure favourites directory exists
        if !favourites_dir.exists() {
            fs::create_dir_all(&favourites_dir).with_context(|| {
                format!(
                    "Failed to create favourites directory: {}",
                    favourites_dir.display()
                )
            })?;
        }

        Ok(Self { favourites_dir })
    }

    /// Get the path to a provider's favourites file
    fn get_favourites_path(&self, provider_hash: &str) -> PathBuf {
        self.favourites_dir.join(format!("{}.json", provider_hash))
    }

    /// Load favourites for a specific provider
    pub fn get_favourites(&self, provider_hash: &str) -> Result<Vec<FavouriteStream>> {
        let path = self.get_favourites_path(provider_hash);

        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read favourites file: {}", path.display()))?;

        let data: FavouritesData =
            serde_json::from_str(&content).with_context(|| "Failed to parse favourites JSON")?;

        Ok(data.favourites)
    }

    /// Save favourites for a specific provider
    pub fn save_favourites(
        &self,
        provider_hash: &str,
        favourites: Vec<FavouriteStream>,
    ) -> Result<()> {
        let path = self.get_favourites_path(provider_hash);
        let data = FavouritesData { favourites };

        let content = serde_json::to_string_pretty(&data)
            .with_context(|| "Failed to serialize favourites")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write favourites file: {}", path.display()))?;

        Ok(())
    }

    /// Add a favourite
    pub fn add_favourite(&self, provider_hash: &str, favourite: FavouriteStream) -> Result<()> {
        let mut favourites = self.get_favourites(provider_hash)?;

        // Check if already exists
        if !favourites
            .iter()
            .any(|f| f.stream_id == favourite.stream_id && f.stream_type == favourite.stream_type)
        {
            favourites.push(favourite);
            self.save_favourites(provider_hash, favourites)?;
        }

        Ok(())
    }

    /// Remove a favourite
    pub fn remove_favourite(
        &self,
        provider_hash: &str,
        stream_id: u32,
        stream_type: &str,
    ) -> Result<()> {
        let mut favourites = self.get_favourites(provider_hash)?;
        favourites.retain(|f| !(f.stream_id == stream_id && f.stream_type == stream_type));
        self.save_favourites(provider_hash, favourites)?;
        Ok(())
    }

    /// Check if a stream is a favourite
    pub fn is_favourite(
        &self,
        provider_hash: &str,
        stream_id: u32,
        stream_type: &str,
    ) -> Result<bool> {
        let favourites = self.get_favourites(provider_hash)?;
        Ok(favourites
            .iter()
            .any(|f| f.stream_id == stream_id && f.stream_type == stream_type))
    }
}
