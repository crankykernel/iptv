// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::cache::{CacheManager, CacheMetadata};
use crate::favourites::FavouritesManager;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, warn};

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Value = Deserialize::deserialize(deserializer)?;

    match value {
        Value::Array(arr) => {
            let strings: Vec<String> = arr
                .into_iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s),
                    Value::Null => None, // Skip null values
                    _ => None,           // Skip other non-string values
                })
                .collect();
            if strings.is_empty() {
                Ok(None)
            } else {
                Ok(Some(strings))
            }
        }
        Value::String(s) => {
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(vec![s]))
            }
        }
        Value::Null => Ok(None),
        _ => Err(D::Error::custom("Expected string or array")),
    }
}

fn deserialize_number_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Value = Deserialize::deserialize(deserializer)?;

    match value {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(D::Error::custom("Expected string or number")),
    }
}

fn deserialize_optional_number_as_string<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Value = Deserialize::deserialize(deserializer)?;

    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s)),
        Value::Number(n) => Ok(Some(n.to_string())),
        _ => Err(D::Error::custom("Expected string, number, or null")),
    }
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
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub exp_date: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub is_trial: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub active_cons: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub created_at: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub max_connections: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub url: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub port: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub https_port: String,
    pub server_protocol: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
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
    pub category_id: Option<String>,
    #[serde(default)]
    pub category_ids: Option<Vec<u32>>,
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
pub struct FavouriteStream {
    pub stream_id: u32,
    pub name: String,
    pub stream_type: String,
    pub provider_hash: String,
    pub added_date: chrono::DateTime<chrono::Utc>,
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VodInfoResponse {
    pub info: VodInfo,
    pub movie_data: MovieData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VodInfo {
    #[serde(default)]
    pub movie_image: Option<String>,
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_optional_number_as_string")]
    pub tmdb_id: Option<String>,
    #[serde(default)]
    pub backdrop: Option<String>,
    #[serde(default)]
    pub youtube_trailer: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default)]
    pub cast: Option<String>,
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub releasedate: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub backdrop_path: Option<Vec<String>>,
    #[serde(default)]
    pub duration_secs: Option<Value>,
    #[serde(default)]
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieData {
    pub stream_id: u32,
    pub name: String,
    #[serde(default, deserialize_with = "deserialize_optional_number_as_string")]
    pub added: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    pub container_extension: String,
    #[serde(default)]
    pub custom_sid: Option<String>,
    #[serde(default)]
    pub direct_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesInfo {
    #[serde(default)]
    pub num: u32,
    pub name: String,
    pub series_id: u32,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default)]
    pub cast: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(rename = "releaseDate", default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_number_as_string")]
    pub rating: Option<String>,
    #[serde(default)]
    pub rating_5based: Option<Value>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub backdrop_path: Option<Vec<String>>,
    #[serde(default)]
    pub youtube_trailer: Option<String>,
    #[serde(default)]
    pub episode_run_time: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    // Additional fields that might appear in series responses
    #[serde(default)]
    pub category_ids: Option<Vec<u32>>,
    #[serde(default)]
    pub added: Option<String>,
    #[serde(default)]
    pub is_adult: Option<Value>,
    #[serde(default)]
    pub stream_type: Option<String>,
    #[serde(default)]
    pub stream_icon: Option<String>,
    #[serde(default)]
    pub epg_channel_id: Option<Value>,
    #[serde(default)]
    pub custom_sid: Option<String>,
    #[serde(default)]
    pub tv_archive: Option<Value>,
    #[serde(default)]
    pub direct_source: Option<String>,
    #[serde(default)]
    pub tv_archive_duration: Option<Value>,
    #[serde(default)]
    pub stream_id: Option<u32>,
    #[serde(default)]
    pub tmdb: Option<String>,
}

// Series info object that comes inside the series detail response (without series_id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesDetailInfo {
    pub name: String,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default)]
    pub cast: Option<String>,
    #[serde(default)]
    pub director: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_number_as_string")]
    pub rating: Option<String>,
    #[serde(default)]
    pub rating_5based: Option<Value>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub backdrop_path: Option<Vec<String>>,
    #[serde(default)]
    pub youtube_trailer: Option<String>,
    #[serde(default)]
    pub episode_run_time: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub episode_num: u32,
    pub title: String,
    #[serde(default)]
    pub container_extension: Option<String>,
    #[serde(default)]
    pub info: Option<EpisodeInfo>,
    #[serde(default)]
    pub custom_sid: Option<String>,
    #[serde(default)]
    pub added: Option<String>,
    #[serde(default)]
    pub season: u32,
    #[serde(default)]
    pub direct_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInfo {
    #[serde(default)]
    pub tmdb_id: Option<u32>,
    #[serde(default)]
    pub releasedate: Option<String>,
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default, rename = "durationSecs")]
    pub duration_secs: Option<u32>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub movie_image: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_number_as_string")]
    pub rating: Option<String>,
}

// Actual API response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesInfoResponse {
    #[serde(default)]
    pub info: Option<SeriesDetailInfo>, // Use the new struct without series_id
    #[serde(default)]
    pub seasons: Vec<ApiSeason>, // Direct seasons array
    #[serde(default)]
    pub episodes: Option<std::collections::HashMap<String, Vec<ApiEpisode>>>, // Episodes by season
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSeason {
    pub name: String,
    #[serde(deserialize_with = "deserialize_number_as_string")]
    pub episode_count: String,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub air_date: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub cover_tmdb: Option<String>,
    pub season_number: u32,
    #[serde(default)]
    pub cover_big: Option<String>,
    #[serde(default, rename = "releaseDate")]
    pub release_date: Option<String>,
    #[serde(default)]
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEpisode {
    pub id: String,
    pub episode_num: u32,
    pub title: String,
    #[serde(default)]
    pub container_extension: Option<String>,
    #[serde(default)]
    pub info: Option<EpisodeInfo>,
    #[serde(default)]
    pub custom_sid: Option<String>,
    #[serde(default)]
    pub added: Option<String>,
    #[serde(default)]
    pub season: u32,
    #[serde(default)]
    pub direct_source: Option<String>,
}

// Keep the old structures for compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Season {
    #[serde(default)]
    pub season_number: u32,
    #[serde(default)]
    pub name: Option<String>,
    pub episodes: Vec<Episode>,
}

pub struct XTreamAPI {
    client: Client,
    base_url: String,
    username: String,
    password: String,
    provider_name: Option<String>,
    pub cache_manager: CacheManager,
    pub favourites_manager: FavouritesManager,
    pub provider_hash: String,
    pub logger: Option<Box<dyn Fn(String) + Send + Sync>>,
    pub show_progress: bool,
}

impl std::fmt::Debug for XTreamAPI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XTreamAPI")
            .field("base_url", &self.base_url)
            .field("username", &self.username)
            .field("provider_name", &self.provider_name)
            .field("provider_hash", &self.provider_hash)
            .field("show_progress", &self.show_progress)
            .field("logger", &self.logger.is_some())
            .finish()
    }
}

impl XTreamAPI {
    pub fn new(
        server_url: String,
        username: String,
        password: String,
        provider_name: Option<String>,
    ) -> Result<Self> {
        let url = reqwest::Url::parse(&server_url).with_context(|| "Invalid server URL")?;

        let base_url = if let Some(port) = url.port() {
            format!(
                "{}://{}:{}",
                url.scheme(),
                url.host_str().unwrap_or("localhost"),
                port
            )
        } else {
            format!(
                "{}://{}",
                url.scheme(),
                url.host_str().unwrap_or("localhost")
            )
        };

        let mut cache_manager = CacheManager::new()?;
        let provider_hash = cache_manager.get_provider_hash(&base_url, provider_name.as_deref())?;
        let favourites_manager = FavouritesManager::new()?;

        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("Mozilla/5.0")
                .build()?,
            base_url: base_url.clone(),
            username,
            password,
            provider_name,
            cache_manager,
            favourites_manager,
            provider_hash,
            logger: None,
            show_progress: true,
        })
    }

    pub fn set_logger(&mut self, logger: Box<dyn Fn(String) + Send + Sync>) {
        self.logger = Some(logger);
        self.show_progress = false;
    }

    pub fn disable_progress(&mut self) {
        self.show_progress = false;
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

        // Create a friendly action description
        let action_desc = match action {
            "get_live_categories" => "live categories",
            "get_vod_categories" => "VOD categories",
            "get_series_categories" => "series categories",
            "get_live_streams" => "live streams",
            "get_vod_streams" => "VOD streams",
            "get_series" => "series",
            "get_series_info" => "series info",
            "get_vod_info" => "VOD info",
            "get_user_info" => "user info",
            _ => action,
        };

        let provider_name = self.provider_name.as_deref().unwrap_or("provider");

        if let Some(ref logger) = self.logger {
            logger(format!("Refreshing {} {}", provider_name, action_desc));
        }

        // Create progress bar only if not in TUI mode
        let pb = if self.show_progress && self.logger.is_none() {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} Refreshing {msg} [{elapsed_precise}]")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            pb.set_message(format!("{} {}", provider_name, action_desc));
            Some(pb)
        } else {
            if let Some(ref logger) = self.logger {
                logger(format!("Refreshing {} {}", provider_name, action_desc));
            }
            None
        };

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to send request to {}", url))?;

        if !response.status().is_success() {
            if let Some(pb) = &pb {
                pb.finish_with_message(format!(
                    "✗ {} {} - HTTP {}",
                    provider_name,
                    action_desc,
                    response.status()
                ));
            }
            return Err(anyhow::anyhow!(
                "HTTP request failed with status: {}",
                response.status()
            ));
        }

        // Don't update message during download, keep the action description

        // Stream the response and track bytes
        let mut response_bytes = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = futures_util::StreamExt::next(&mut stream).await {
            let chunk = chunk_result.with_context(|| "Failed to read response chunk")?;

            response_bytes.extend_from_slice(&chunk);

            if let Some(pb) = &pb {
                pb.set_position(response_bytes.len() as u64);

                // Format bytes nicely
                let bytes_str = if response_bytes.len() < 1024 {
                    format!("{} B", response_bytes.len())
                } else if response_bytes.len() < 1024 * 1024 {
                    format!("{:.1} KB", response_bytes.len() as f64 / 1024.0)
                } else {
                    format!("{:.1} MB", response_bytes.len() as f64 / (1024.0 * 1024.0))
                };

                pb.set_message(format!("{} {} - {}", provider_name, action_desc, bytes_str));
            }
        }

        // Don't show parsing message anymore, keep the action description

        if response_bytes.is_empty() {
            if let Some(pb) = &pb {
                pb.finish_with_message(format!(
                    "✗ {} {} - empty response",
                    provider_name, action_desc
                ));
            }
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        let response_size = response_bytes.len();
        let response_text = String::from_utf8(response_bytes)
            .with_context(|| "Failed to convert response to UTF-8 string")?;

        if response_text.trim().is_empty() {
            if let Some(pb) = &pb {
                pb.finish_with_message(format!(
                    "✗ {} {} - empty response",
                    provider_name, action_desc
                ));
            }
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        let json_result: Result<T> = serde_json::from_str(&response_text).map_err(|e| {
            // Get detailed error information with character position
            let error_msg = {
                let line_num = e.line();
                let col_num = e.column();

                // Calculate byte position approximately
                let lines: Vec<&str> = response_text.lines().collect();
                let mut byte_pos = 0;
                for (i, line_content) in lines.iter().enumerate() {
                    if i + 1 == line_num {
                        byte_pos += col_num.saturating_sub(1);
                        break;
                    }
                    byte_pos += line_content.len() + 1; // +1 for newline
                }

                // Get context around the error (100 chars before and after)
                let start = byte_pos.saturating_sub(100);
                let end = std::cmp::min(byte_pos + 100, response_text.len());
                let context = &response_text[start..end];

                format!(
                    "JSON parsing failed at line {}, column {} (byte position ~{}):\n\
                    Context: ...{}...\n\
                    Error: {}",
                    line_num,
                    col_num,
                    byte_pos,
                    context.replace(['\n', '\r'], " "),
                    e
                )
            };

            warn!("JSON parsing error: {}", error_msg);
            anyhow::anyhow!(error_msg)
        });

        let json = match json_result {
            Ok(j) => j,
            Err(e) => {
                if let Some(pb) = &pb {
                    pb.finish_with_message(format!(
                        "✗ {} {} - parse error",
                        provider_name, action_desc
                    ));
                }
                return Err(e);
            }
        };

        if let Some(pb) = pb {
            // Format final size
            let bytes_str = if response_size < 1024 {
                format!("{} B", response_size)
            } else if response_size < 1024 * 1024 {
                format!("{:.1} KB", response_size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", response_size as f64 / (1024.0 * 1024.0))
            };
            pb.finish_with_message(format!(
                "✓ {} {} - {}",
                provider_name, action_desc, bytes_str
            ));
        }
        Ok(json)
    }

    pub async fn get_user_info(&mut self) -> Result<UserInfo> {
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<UserInfo>(&self.provider_hash, "user_info", None)
            .await
        {
            return Ok(cached);
        }

        let response: UserInfoResponse = self.make_request("get_user_info", None).await?;
        let user_info = response.user_info;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "user_info",
                None,
                user_info.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache user info: {}", e);
        }

        Ok(user_info)
    }

    pub async fn get_live_categories(&mut self) -> Result<Vec<Category>> {
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "live_categories", None)
            .await
        {
            return Ok(cached);
        }

        let categories: Vec<Category> = self.make_request("get_live_categories", None).await?;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "live_categories",
                None,
                categories.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache live categories: {}", e);
        }

        Ok(categories)
    }

    pub async fn get_vod_categories(&mut self) -> Result<Vec<Category>> {
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "vod_categories", None)
            .await
        {
            return Ok(cached);
        }

        let categories: Vec<Category> = self.make_request("get_vod_categories", None).await?;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "vod_categories",
                None,
                categories.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache vod categories: {}", e);
        }

        Ok(categories)
    }

    pub async fn get_series_categories(&mut self) -> Result<Vec<Category>> {
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "series_categories", None)
            .await
        {
            return Ok(cached);
        }

        let categories: Vec<Category> = self.make_request("get_series_categories", None).await?;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "series_categories",
                None,
                categories.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache series categories: {}", e);
        }

        Ok(categories)
    }

    pub async fn get_live_streams(&mut self, category_id: Option<&str>) -> Result<Vec<Stream>> {
        // Try to get from "All" cache first and filter if needed
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Stream>>(&self.provider_hash, "live_streams", None)
            .await
        {
            let filtered_streams = if let Some(cat_id) = category_id {
                cached
                    .into_iter()
                    .filter(|stream| {
                        stream
                            .category_id
                            .as_ref()
                            .map(|id| id == cat_id)
                            .unwrap_or(false)
                    })
                    .collect()
            } else {
                cached
            };
            return Ok(filtered_streams);
        }

        // If All cache is expired or missing, fetch fresh data
        let streams: Vec<Stream> = self.make_request("get_live_streams", None).await?;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        // Always cache the full "All" response
        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "live_streams",
                None,
                streams.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache live streams: {}", e);
        }

        // Return filtered result if category was specified
        let result_streams = if let Some(cat_id) = category_id {
            streams
                .into_iter()
                .filter(|stream| {
                    stream
                        .category_id
                        .as_ref()
                        .map(|id| id == cat_id)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            streams
        };

        Ok(result_streams)
    }

    pub async fn get_vod_streams(&mut self, category_id: Option<&str>) -> Result<Vec<Stream>> {
        // Try to get from "All" cache first and filter if needed
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Stream>>(&self.provider_hash, "vod_streams", None)
            .await
        {
            let filtered_streams = if let Some(cat_id) = category_id {
                cached
                    .into_iter()
                    .filter(|stream| {
                        stream
                            .category_id
                            .as_ref()
                            .map(|id| id == cat_id)
                            .unwrap_or(false)
                    })
                    .collect()
            } else {
                cached
            };
            return Ok(filtered_streams);
        }

        // If All cache is expired or missing, fetch fresh data
        let streams: Vec<Stream> = self.make_request("get_vod_streams", None).await?;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        // Always cache the full "All" response
        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "vod_streams",
                None,
                streams.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache vod streams: {}", e);
        }

        // Return filtered result if category was specified
        let result_streams = if let Some(cat_id) = category_id {
            streams
                .into_iter()
                .filter(|stream| {
                    stream
                        .category_id
                        .as_ref()
                        .map(|id| id == cat_id)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            streams
        };

        Ok(result_streams)
    }

    pub async fn get_series(&mut self, category_id: Option<&str>) -> Result<Vec<SeriesInfo>> {
        // Try to get from "All" cache first and filter if needed
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<SeriesInfo>>(&self.provider_hash, "series", None)
            .await
        {
            let filtered_series = if let Some(cat_id) = category_id {
                cached
                    .into_iter()
                    .filter(|series| {
                        series
                            .category_id
                            .as_ref()
                            .map(|id| id == cat_id)
                            .unwrap_or(false)
                    })
                    .collect()
            } else {
                cached
            };
            return Ok(filtered_series);
        }

        // If All cache is expired or missing, fetch fresh data
        let series: Vec<SeriesInfo> = self.make_request("get_series", None).await?;

        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        // Always cache the full "All" response
        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                "series",
                None,
                series.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache series: {}", e);
        }

        // Return filtered result if category was specified
        let result_series = if let Some(cat_id) = category_id {
            series
                .into_iter()
                .filter(|series| {
                    series
                        .category_id
                        .as_ref()
                        .map(|id| id == cat_id)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            series
        };

        Ok(result_series)
    }

    pub async fn get_series_info(&mut self, series_id: u32) -> Result<SeriesInfoResponse> {
        // Try to get from cache first
        let cache_key = format!("series_info_{}", series_id);
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<SeriesInfoResponse>(&self.provider_hash, &cache_key, None)
            .await
        {
            return Ok(cached);
        }

        // Fetch fresh data from API
        let url = format!(
            "{}/player_api.php?username={}&password={}&action=get_series_info&series_id={}",
            self.base_url, self.username, self.password, series_id
        );

        debug!("Requesting series info for ID: {}", series_id);

        if let Some(ref logger) = self.logger {
            logger(format!("Fetching series info for ID: {}", series_id));
        }

        let response = self.client.get(&url).send().await?;
        let response_text = response.text().await?;

        if response_text.trim().is_empty() {
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        // Log response for debugging
        debug!(
            "Series info response: {}",
            if response_text.len() > 1000 {
                format!(
                    "{}... (truncated, {} bytes total)",
                    &response_text[..1000],
                    response_text.len()
                )
            } else {
                response_text.clone()
            }
        );

        let series_data: SeriesInfoResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                warn!("JSON parsing error for series info: {}", e);
                warn!(
                    "Response content: {}",
                    if response_text.len() > 200 {
                        format!("{}... (truncated)", &response_text[..200])
                    } else {
                        response_text.clone()
                    }
                );
                anyhow::anyhow!("Failed to parse series info: {}", e)
            })?;

        // Cache the result
        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                &cache_key,
                None,
                series_data.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache series info: {}", e);
        }

        Ok(series_data)
    }

    pub async fn get_vod_info(&mut self, vod_id: u32) -> Result<VodInfoResponse> {
        // Try to get from cache first
        let cache_key = format!("vod_info_{}", vod_id);
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<VodInfoResponse>(&self.provider_hash, &cache_key, None)
            .await
        {
            return Ok(cached);
        }

        // Fetch fresh data from API
        let url = format!(
            "{}/player_api.php?username={}&password={}&action=get_vod_info&vod_id={}",
            self.base_url, self.username, self.password, vod_id
        );

        debug!("Requesting VOD info for ID: {}", vod_id);

        if let Some(ref logger) = self.logger {
            logger(format!("Fetching movie info for ID: {}", vod_id));
        }

        let response = self.client.get(&url).send().await?;
        let response_text = response.text().await?;

        if response_text.trim().is_empty() {
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        // Log response for debugging
        debug!(
            "VOD info response: {}",
            if response_text.len() > 1000 {
                format!(
                    "{}... (truncated, {} bytes total)",
                    &response_text[..1000],
                    response_text.len()
                )
            } else {
                response_text.clone()
            }
        );

        let vod_data: VodInfoResponse = serde_json::from_str(&response_text).map_err(|e| {
            warn!("JSON parsing error for VOD info: {}", e);
            warn!(
                "Response content: {}",
                if response_text.len() > 200 {
                    format!("{}... (truncated)", &response_text[..200])
                } else {
                    response_text.clone()
                }
            );
            anyhow::anyhow!("Failed to parse VOD info: {}", e)
        })?;

        // Cache the result
        let metadata = CacheMetadata::new(self.base_url.clone(), self.provider_name.clone());

        if let Err(e) = self
            .cache_manager
            .store_cache(
                &self.provider_hash,
                &cache_key,
                None,
                vod_data.clone(),
                metadata,
            )
            .await
        {
            eprintln!("Warning: Failed to cache VOD info: {}", e);
        }

        Ok(vod_data)
    }

    pub fn get_episode_stream_url(&self, episode_id: &str, extension: Option<&str>) -> String {
        let ext = extension.unwrap_or("m3u8");
        format!(
            "{}/series/{}/{}/{}.{}",
            self.base_url, self.username, self.password, episode_id, ext
        )
    }

    pub fn get_stream_url(
        &self,
        stream_id: u32,
        stream_type: &str,
        extension: Option<&str>,
    ) -> String {
        let ext = extension.unwrap_or("m3u8");

        // URL logging moved to TUI logs panel
        match stream_type {
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
        }
    }

    pub async fn clear_cache(&mut self) -> Result<()> {
        self.cache_manager
            .clear_provider_cache(&self.provider_hash)
            .await
    }

    pub async fn refresh_cache(&mut self) -> Result<()> {
        // Clear existing cache first to force refresh
        self.clear_cache().await?;
        self.warm_cache().await
    }

    pub async fn warm_cache(&mut self) -> Result<()> {
        debug!("Warming cache for provider...");

        // Warm categories first
        let mut tasks = Vec::new();

        // Warm live categories
        if let Ok(Some(_cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "live_categories", None)
            .await
        {
            // Cache exists, no need to warm
        } else {
            tasks.push("live_categories");
        }

        // Warm VOD categories
        if let Ok(Some(_cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "vod_categories", None)
            .await
        {
            // Cache exists, no need to warm
        } else {
            tasks.push("vod_categories");
        }

        // Warm series categories
        if let Ok(Some(_cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "series_categories", None)
            .await
        {
            // Cache exists, no need to warm
        } else {
            tasks.push("series_categories");
        }

        // Warm categories
        for task in tasks {
            let result = match task {
                "live_categories" => self.get_live_categories().await.map(|_| ()),
                "vod_categories" => self.get_vod_categories().await.map(|_| ()),
                "series_categories" => self.get_series_categories().await.map(|_| ()),
                _ => continue,
            };

            if let Err(e) = result {
                eprintln!("Warning: Failed to warm {}: {}", task, e);
            }
        }

        // Now warm ONLY the "All" streams/series (no individual categories)
        let mut warmed_count = 0;
        let content_types = ["live", "vod", "series"];

        debug!(
            "Warming 'All' streams for {} content types...",
            content_types.len()
        );

        for content_type in content_types {
            // Check if All cache already exists and is fresh
            let cache_key = match content_type {
                "live" => "live_streams",
                "vod" => "vod_streams",
                "series" => "series",
                _ => continue,
            };

            let is_cached_and_fresh = match content_type {
                "live" | "vod" => {
                    if let Ok(Some(_cached)) = self
                        .cache_manager
                        .get_cached::<Vec<Stream>>(&self.provider_hash, cache_key, None)
                        .await
                    {
                        true // Cache exists
                    } else {
                        false
                    }
                }
                "series" => {
                    if let Ok(Some(_cached)) = self
                        .cache_manager
                        .get_cached::<Vec<SeriesInfo>>(&self.provider_hash, cache_key, None)
                        .await
                    {
                        true // Cache exists
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if is_cached_and_fresh {
                continue;
            }

            let result = match content_type {
                "live" => self.get_live_streams(None).await.map(|_| ()),
                "vod" => self.get_vod_streams(None).await.map(|_| ()),
                "series" => self.get_series(None).await.map(|_| ()),
                _ => continue,
            };

            match result {
                Ok(()) => {
                    warmed_count += 1;
                    debug!("Warmed 'All' {} streams", content_type);
                }
                Err(e) => {
                    warn!("Failed to warm {} 'All' streams: {}", content_type, e);
                }
            }
        }

        debug!(
            "Cache warming complete! Warmed {} 'All' content types.",
            warmed_count
        );
        Ok(())
    }
}
