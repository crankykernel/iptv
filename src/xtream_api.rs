// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.and_then(|s| if s.is_empty() { None } else { Some(s) }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfoResponse {
    pub user_info: UserInfo,
    pub server_info: ServerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub username: String,
    pub password: String,
    pub message: String,
    pub auth: u8,
    pub status: String,
    pub exp_date: String,
    pub is_trial: String,
    pub active_cons: String,
    pub created_at: String,
    pub max_connections: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub url: String,
    pub port: String,
    pub https_port: String,
    pub server_protocol: String,
    pub rtmp_port: String,
    pub timezone: String,
    pub timestamp_now: u64,
    pub time_now: String,
    pub process: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub category_id: String,
    pub category_name: String,
    pub parent_id: Option<u32>,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.category_name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub num: u32,
    pub name: String,
    pub stream_type: String,
    pub stream_id: u32,
    #[serde(default)]
    pub stream_icon: Option<String>,
    #[serde(default)]
    pub epg_channel_id: Option<Value>,
    #[serde(default)]
    pub added: Option<String>,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub custom_sid: Option<String>,
    #[serde(default)]
    pub tv_archive: Option<Value>,
    #[serde(default)]
    pub direct_source: Option<String>,
    #[serde(default)]
    pub tv_archive_duration: Option<Value>,
    #[serde(default)]
    pub is_adult: Option<Value>,
    // VOD-specific fields
    #[serde(default)]
    pub rating: Option<Value>,
    #[serde(default)]
    pub rating_5based: Option<Value>,
    #[serde(default)]
    pub container_extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesInfo {
    pub num: u32,
    pub name: String,
    pub series_id: u32,
    pub cover: Option<String>,
    pub plot: Option<String>,
    pub cast: Option<String>,
    pub director: Option<String>,
    pub genre: Option<String>,
    pub release_date: Option<String>,
    pub last_modified: Option<String>,
    pub rating: Option<String>,
    pub rating_5based: Option<f32>,
    pub backdrop_path: Option<Vec<String>>,
    pub youtube_trailer: Option<String>,
    pub episode_run_time: Option<String>,
    pub category_id: String,
}

#[derive(Debug)]
pub struct CacheEntry<T> {
    pub data: T,
    pub timestamp: Instant,
    pub ttl: Duration,
}

impl<T> CacheEntry<T> {
    pub fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            timestamp: Instant::now(),
            ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

#[derive(Debug)]
pub struct XTreamAPI {
    client: Client,
    base_url: String,
    username: String,
    password: String,
    cache_ttl: Duration,
    
    // Cache storage
    user_info_cache: Option<CacheEntry<UserInfo>>,
    categories_cache: HashMap<String, CacheEntry<Vec<Category>>>,
    streams_cache: HashMap<String, CacheEntry<Vec<Stream>>>,
    series_cache: HashMap<String, CacheEntry<Vec<SeriesInfo>>>,
}

impl XTreamAPI {
    pub fn new(server_url: String, username: String, password: String, cache_ttl_seconds: u64) -> Result<Self> {
        let url = reqwest::Url::parse(&server_url)
            .with_context(|| "Invalid server URL")?;
        
        let base_url = if let Some(port) = url.port() {
            format!("{}://{}:{}", url.scheme(), url.host_str().unwrap_or("localhost"), port)
        } else {
            format!("{}://{}", url.scheme(), url.host_str().unwrap_or("localhost"))
        };

        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("Mozilla/5.0")
                .build()?,
            base_url: base_url.clone(),
            username,
            password,
            cache_ttl: Duration::from_secs(cache_ttl_seconds),
            user_info_cache: None,
            categories_cache: HashMap::new(),
            streams_cache: HashMap::new(),
            series_cache: HashMap::new(),
        })
    }

    async fn make_request<T>(&self, action: &str, category_id: Option<&str>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut url = format!(
            "{}/player_api.php?username={}&password={}&action={}",
            self.base_url, self.username, self.password, action
        );

        if let Some(cat_id) = category_id {
            url.push_str(&format!("&category_id={}", cat_id));
        }

        println!("Requesting: {} (action: {}, category: {:?})", url, action, category_id);

        // Create progress bar
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner())
        );
        pb.set_message("Sending request...");

        let response = self.client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        if !response.status().is_success() {
            pb.finish_and_clear();
            return Err(anyhow::anyhow!(
                "HTTP request failed with status: {}",
                response.status()
            ));
        }

        pb.set_message("Downloading response...");

        let response_text = response
            .text()
            .await
            .with_context(|| "Failed to get response text")?;

        pb.set_message("Parsing JSON...");

        println!("Response size: {} bytes", response_text.len());

        if response_text.trim().is_empty() {
            pb.finish_and_clear();
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        let json: T = serde_json::from_str(&response_text)
            .with_context(|| {
                let truncated_response = if response_text.len() > 500 {
                    format!("{}... (truncated {} bytes)", &response_text[..500], response_text.len())
                } else {
                    response_text.clone()
                };
                format!("Failed to parse JSON response: {}", truncated_response)
            })?;

        pb.finish_and_clear();
        Ok(json)
    }

    pub async fn get_user_info(&mut self) -> Result<UserInfo> {
        if let Some(ref entry) = self.user_info_cache {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let response: UserInfoResponse = self.make_request("get_user_info", None).await?;
        let user_info = response.user_info;
        self.user_info_cache = Some(CacheEntry::new(user_info.clone(), self.cache_ttl));
        
        Ok(user_info)
    }

    pub async fn get_live_categories(&mut self) -> Result<Vec<Category>> {
        let cache_key = "live".to_string();
        
        if let Some(entry) = self.categories_cache.get(&cache_key) {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let categories: Vec<Category> = self.make_request("get_live_categories", None).await?;
        self.categories_cache.insert(
            cache_key, 
            CacheEntry::new(categories.clone(), self.cache_ttl)
        );
        
        Ok(categories)
    }

    pub async fn get_vod_categories(&mut self) -> Result<Vec<Category>> {
        let cache_key = "vod".to_string();
        
        if let Some(entry) = self.categories_cache.get(&cache_key) {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let categories: Vec<Category> = self.make_request("get_vod_categories", None).await?;
        self.categories_cache.insert(
            cache_key,
            CacheEntry::new(categories.clone(), self.cache_ttl)
        );
        
        Ok(categories)
    }

    pub async fn get_series_categories(&mut self) -> Result<Vec<Category>> {
        let cache_key = "series".to_string();
        
        if let Some(entry) = self.categories_cache.get(&cache_key) {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let categories: Vec<Category> = self.make_request("get_series_categories", None).await?;
        self.categories_cache.insert(
            cache_key,
            CacheEntry::new(categories.clone(), self.cache_ttl)
        );
        
        Ok(categories)
    }

    pub async fn get_live_streams(&mut self, category_id: Option<&str>) -> Result<Vec<Stream>> {
        let cache_key = format!("live_{}", category_id.unwrap_or("all"));
        
        if let Some(entry) = self.streams_cache.get(&cache_key) {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let streams: Vec<Stream> = self.make_request("get_live_streams", category_id).await?;
        self.streams_cache.insert(
            cache_key,
            CacheEntry::new(streams.clone(), self.cache_ttl)
        );
        
        Ok(streams)
    }

    pub async fn get_vod_streams(&mut self, category_id: Option<&str>) -> Result<Vec<Stream>> {
        let cache_key = format!("vod_{}", category_id.unwrap_or("all"));
        
        if let Some(entry) = self.streams_cache.get(&cache_key) {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let streams: Vec<Stream> = self.make_request("get_vod_streams", category_id).await?;
        self.streams_cache.insert(
            cache_key,
            CacheEntry::new(streams.clone(), self.cache_ttl)
        );
        
        Ok(streams)
    }

    pub async fn get_series(&mut self, category_id: Option<&str>) -> Result<Vec<SeriesInfo>> {
        let cache_key = format!("series_{}", category_id.unwrap_or("all"));
        
        if let Some(entry) = self.series_cache.get(&cache_key) {
            if !entry.is_expired() {
                return Ok(entry.data.clone());
            }
        }

        let series: Vec<SeriesInfo> = self.make_request("get_series", category_id).await?;
        self.series_cache.insert(
            cache_key,
            CacheEntry::new(series.clone(), self.cache_ttl)
        );
        
        Ok(series)
    }

    pub fn get_stream_url(&self, stream_id: u32, stream_type: &str, extension: Option<&str>) -> String {
        let ext = extension.unwrap_or("m3u8");
        let url = match stream_type {
            "live" => format!(
                "{}/live/{}/{}/{}.{}",
                self.base_url, self.username, self.password, stream_id, ext
            ),
            "movie" => format!(
                "{}/movie/{}/{}/{}.{}",
                self.base_url, self.username, self.password, stream_id, ext
            ),
            "series" => format!(
                "{}/series/{}/{}/{}.{}",
                self.base_url, self.username, self.password, stream_id, ext
            ),
            _ => format!(
                "{}/live/{}/{}/{}.{}",
                self.base_url, self.username, self.password, stream_id, ext
            ),
        };
        
        println!("Stream URL: {}", url);
        url
    }

    pub fn clear_cache(&mut self) {
        self.user_info_cache = None;
        self.categories_cache.clear();
        self.streams_cache.clear();
        self.series_cache.clear();
    }
}


