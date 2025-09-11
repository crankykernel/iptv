// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs as async_fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub created_at: u64,
    pub provider_url: String,
    pub provider_name: Option<String>,
}

impl CacheMetadata {
    pub fn new(provider_url: String, provider_name: Option<String>) -> Self {
        Self {
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            provider_url,
            provider_name,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedData<T> {
    pub(crate) metadata: CacheMetadata,
    pub(crate) data: T,
}

impl<T> CachedData<T> {
    fn new(data: T, metadata: CacheMetadata) -> Self {
        Self { metadata, data }
    }
}

#[derive(Debug)]
pub struct CacheManager {
    cache_dir: PathBuf,
    provider_index: HashMap<String, String>,
}

impl CacheManager {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
            .join("iptv");

        let mut manager = Self {
            cache_dir,
            provider_index: HashMap::new(),
        };

        manager.ensure_cache_dir_exists()?;
        manager.load_provider_index()?;

        Ok(manager)
    }

    fn ensure_cache_dir_exists(&self) -> Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir).with_context(|| {
                format!(
                    "Failed to create cache directory: {}",
                    self.cache_dir.display()
                )
            })?;
        }

        let providers_dir = self.cache_dir.join("providers");
        if !providers_dir.exists() {
            fs::create_dir_all(&providers_dir).with_context(|| {
                format!(
                    "Failed to create providers directory: {}",
                    providers_dir.display()
                )
            })?;
        }

        Ok(())
    }

    fn load_provider_index(&mut self) -> Result<()> {
        let index_path = self.cache_dir.join("index.json");
        if index_path.exists() {
            let content = fs::read_to_string(&index_path).with_context(|| {
                format!("Failed to read provider index: {}", index_path.display())
            })?;
            self.provider_index = serde_json::from_str(&content)
                .with_context(|| "Failed to parse provider index JSON")?;
        }
        Ok(())
    }

    fn save_provider_index(&self) -> Result<()> {
        let index_path = self.cache_dir.join("index.json");
        let content = serde_json::to_string_pretty(&self.provider_index)
            .with_context(|| "Failed to serialize provider index")?;
        fs::write(&index_path, content)
            .with_context(|| format!("Failed to write provider index: {}", index_path.display()))?;
        Ok(())
    }

    pub fn get_provider_hash(
        &mut self,
        provider_url: &str,
        _provider_name: Option<&str>,
    ) -> Result<String> {
        if let Some(hash) = self.provider_index.get(provider_url) {
            return Ok(hash.clone());
        }

        let mut hasher = Sha256::new();
        hasher.update(provider_url.as_bytes());
        let hash = format!("{:x}", hasher.finalize())[..16].to_string();

        self.provider_index
            .insert(provider_url.to_string(), hash.clone());
        self.save_provider_index()?;

        let provider_dir = self.cache_dir.join("providers").join(&hash);
        if !provider_dir.exists() {
            fs::create_dir_all(&provider_dir).with_context(|| {
                format!(
                    "Failed to create provider cache directory: {}",
                    provider_dir.display()
                )
            })?;
        }

        Ok(hash)
    }

    fn get_cache_path(
        &self,
        provider_hash: &str,
        cache_type: &str,
        category_id: Option<&str>,
    ) -> PathBuf {
        let filename = if let Some(cat_id) = category_id {
            let mut hasher = Sha256::new();
            hasher.update(cat_id.as_bytes());
            let cat_hash = format!("{:x}", hasher.finalize())[..8].to_string();
            format!("{}_{}.json", cache_type, cat_hash)
        } else {
            format!("{}.json", cache_type)
        };

        self.cache_dir
            .join("providers")
            .join(provider_hash)
            .join(filename)
    }

    pub async fn get_cached<T>(
        &self,
        provider_hash: &str,
        cache_type: &str,
        category_id: Option<&str>,
    ) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let cache_path = self.get_cache_path(provider_hash, cache_type, category_id);

        if !cache_path.exists() {
            return Ok(None);
        }

        let content = async_fs::read_to_string(&cache_path)
            .await
            .with_context(|| format!("Failed to read cache file: {}", cache_path.display()))?;

        let cached_data: CachedData<T> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse cache JSON: {}", cache_path.display()))?;

        Ok(Some(cached_data.data))
    }

    pub async fn store_cache<T>(
        &self,
        provider_hash: &str,
        cache_type: &str,
        category_id: Option<&str>,
        data: T,
        metadata: CacheMetadata,
    ) -> Result<()>
    where
        T: Serialize,
    {
        let cache_path = self.get_cache_path(provider_hash, cache_type, category_id);

        if let Some(parent) = cache_path.parent()
            && !parent.exists()
        {
            async_fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create cache directory: {}", parent.display())
            })?;
        }

        let cached_data = CachedData::new(data, metadata);
        let content = serde_json::to_string_pretty(&cached_data)
            .with_context(|| "Failed to serialize cache data")?;

        async_fs::write(&cache_path, content)
            .await
            .with_context(|| format!("Failed to write cache file: {}", cache_path.display()))?;

        Ok(())
    }

    pub async fn clear_provider_cache(&self, provider_hash: &str) -> Result<()> {
        let provider_dir = self.cache_dir.join("providers").join(provider_hash);
        if provider_dir.exists() {
            async_fs::remove_dir_all(&provider_dir)
                .await
                .with_context(|| {
                    format!(
                        "Failed to remove provider cache directory: {}",
                        provider_dir.display()
                    )
                })?;
        }
        Ok(())
    }

    pub async fn clear_all_cache(&self) -> Result<()> {
        let providers_dir = self.cache_dir.join("providers");
        if providers_dir.exists() {
            async_fs::remove_dir_all(&providers_dir)
                .await
                .with_context(|| {
                    format!(
                        "Failed to remove providers cache directory: {}",
                        providers_dir.display()
                    )
                })?;
        }
        self.ensure_cache_dir_exists()?;
        Ok(())
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            eprintln!("Failed to initialize cache manager: {}", e);
            eprintln!("Cache functionality will be disabled");
            Self {
                cache_dir: PathBuf::from("/tmp/iptv_cache_fallback"),
                provider_index: HashMap::new(),
            }
        })
    }
}
