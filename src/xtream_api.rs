// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::cache::{CacheManager, CacheMetadata};
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::time::Duration;

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let value: Value = Deserialize::deserialize(deserializer)?;

    match value {
        Value::Array(arr) => {
            let strings: Result<Vec<String>, _> = arr
                .into_iter()
                .map(|v| match v {
                    Value::String(s) => Ok(s),
                    _ => Err(D::Error::custom("Array contains non-string value")),
                })
                .collect();
            Ok(Some(strings?))
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
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub rating_5based: Option<f32>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub backdrop_path: Option<Vec<String>>,
    #[serde(default)]
    pub youtube_trailer: Option<String>,
    #[serde(default)]
    pub episode_run_time: Option<String>,
    #[serde(default)]
    pub category_id: String,
}

#[derive(Debug)]
pub struct XTreamAPI {
    client: Client,
    base_url: String,
    username: String,
    password: String,
    cache_ttl_seconds: u64,
    provider_name: Option<String>,
    cache_manager: CacheManager,
    provider_hash: String,
}

impl XTreamAPI {
    pub fn new(
        server_url: String,
        username: String,
        password: String,
        cache_ttl_seconds: u64,
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

        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("Mozilla/5.0")
                .build()?,
            base_url: base_url.clone(),
            username,
            password,
            cache_ttl_seconds,
            provider_name,
            cache_manager,
            provider_hash,
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

        println!(
            "Requesting: {} (action: {}, category: {:?})",
            url, action, category_id
        );

        // Create progress bar with indefinite style showing bytes received
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg} [{elapsed_precise}] {bytes}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message("Sending request...");

        let response = self
            .client
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

        pb.set_message("Downloading...");

        // Stream the response and track bytes
        let mut response_bytes = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = futures_util::StreamExt::next(&mut stream).await {
            let chunk = chunk_result.with_context(|| "Failed to read response chunk")?;

            response_bytes.extend_from_slice(&chunk);
            pb.set_position(response_bytes.len() as u64);

            // Format bytes nicely
            let bytes_str = if response_bytes.len() < 1024 {
                format!("{} B", response_bytes.len())
            } else if response_bytes.len() < 1024 * 1024 {
                format!("{:.1} KB", response_bytes.len() as f64 / 1024.0)
            } else {
                format!("{:.1} MB", response_bytes.len() as f64 / (1024.0 * 1024.0))
            };

            let message = format!("Downloading... {}", bytes_str);
            pb.set_message(message);
        }

        pb.set_message("Parsing JSON...");

        println!("Response size: {} bytes", response_bytes.len());

        if response_bytes.is_empty() {
            pb.finish_and_clear();
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        let response_text = String::from_utf8(response_bytes)
            .with_context(|| "Failed to convert response to UTF-8 string")?;

        if response_text.trim().is_empty() {
            pb.finish_and_clear();
            return Err(anyhow::anyhow!("Empty response from server"));
        }

        let json: T = serde_json::from_str(&response_text).with_context(|| {
            let truncated_response = if response_text.len() > 500 {
                format!(
                    "{}... (truncated {} bytes)",
                    &response_text[..500],
                    response_text.len()
                )
            } else {
                response_text.clone()
            };
            format!("Failed to parse JSON response: {}", truncated_response)
        })?;

        pb.finish_and_clear();
        Ok(json)
    }

    pub async fn get_user_info(&mut self) -> Result<UserInfo> {
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<UserInfo>(&self.provider_hash, "user_info", None)
            .await
        {
            if !cached.is_expired() {
                return Ok(cached.data);
            }
        }

        let response: UserInfoResponse = self.make_request("get_user_info", None).await?;
        let user_info = response.user_info;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
            if !cached.is_expired() {
                return Ok(cached.data);
            }
        }

        let categories: Vec<Category> = self.make_request("get_live_categories", None).await?;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
            if !cached.is_expired() {
                return Ok(cached.data);
            }
        }

        let categories: Vec<Category> = self.make_request("get_vod_categories", None).await?;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
            if !cached.is_expired() {
                return Ok(cached.data);
            }
        }

        let categories: Vec<Category> = self.make_request("get_series_categories", None).await?;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
            if !cached.is_expired() {
                let filtered_streams = if let Some(cat_id) = category_id {
                    cached
                        .data
                        .into_iter()
                        .filter(|stream| stream.category_id == cat_id)
                        .collect()
                } else {
                    cached.data
                };
                return Ok(filtered_streams);
            }
        }

        // If All cache is expired or missing, fetch fresh data
        let streams: Vec<Stream> = self.make_request("get_live_streams", None).await?;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
                .filter(|stream| stream.category_id == cat_id)
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
            if !cached.is_expired() {
                let filtered_streams = if let Some(cat_id) = category_id {
                    cached
                        .data
                        .into_iter()
                        .filter(|stream| stream.category_id == cat_id)
                        .collect()
                } else {
                    cached.data
                };
                return Ok(filtered_streams);
            }
        }

        // If All cache is expired or missing, fetch fresh data
        let streams: Vec<Stream> = self.make_request("get_vod_streams", None).await?;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
                .filter(|stream| stream.category_id == cat_id)
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
            if !cached.is_expired() {
                let filtered_series = if let Some(cat_id) = category_id {
                    cached
                        .data
                        .into_iter()
                        .filter(|series| series.category_id == cat_id)
                        .collect()
                } else {
                    cached.data
                };
                return Ok(filtered_series);
            }
        }

        // If All cache is expired or missing, fetch fresh data
        let series: Vec<SeriesInfo> = self.make_request("get_series", None).await?;

        let metadata = CacheMetadata::new(
            self.base_url.clone(),
            self.provider_name.clone(),
            self.cache_ttl_seconds,
        );

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
                .filter(|series| series.category_id == cat_id)
                .collect()
        } else {
            series
        };

        Ok(result_series)
    }

    pub fn get_stream_url(
        &self,
        stream_id: u32,
        stream_type: &str,
        extension: Option<&str>,
    ) -> String {
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

    pub async fn clear_cache(&mut self) -> Result<()> {
        self.cache_manager
            .clear_provider_cache(&self.provider_hash)
            .await
    }

    pub async fn warm_cache(&mut self) -> Result<()> {
        println!("Warming cache for provider...");

        // Warm categories first
        let mut tasks = Vec::new();

        // Warm live categories
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "live_categories", None)
            .await
        {
            if cached.is_expired() {
                tasks.push("live_categories");
            }
        } else {
            tasks.push("live_categories");
        }

        // Warm VOD categories
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "vod_categories", None)
            .await
        {
            if cached.is_expired() {
                tasks.push("vod_categories");
            }
        } else {
            tasks.push("vod_categories");
        }

        // Warm series categories
        if let Ok(Some(cached)) = self
            .cache_manager
            .get_cached::<Vec<Category>>(&self.provider_hash, "series_categories", None)
            .await
        {
            if cached.is_expired() {
                tasks.push("series_categories");
            }
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

        println!(
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
                    if let Ok(Some(cached)) = self
                        .cache_manager
                        .get_cached::<Vec<Stream>>(&self.provider_hash, cache_key, None)
                        .await
                    {
                        !cached.is_expired()
                    } else {
                        false
                    }
                }
                "series" => {
                    if let Ok(Some(cached)) = self
                        .cache_manager
                        .get_cached::<Vec<SeriesInfo>>(&self.provider_hash, cache_key, None)
                        .await
                    {
                        !cached.is_expired()
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
                    println!("Warmed 'All' {} streams", content_type);
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to warm {} 'All' streams: {}",
                        content_type, e
                    );
                }
            }
        }

        println!(
            "Cache warming complete! Warmed {} 'All' content types.",
            warmed_count
        );
        Ok(())
    }
}
