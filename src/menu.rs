// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::config::ProviderConfig;
use crate::player::Player;
use crate::xtream_api::{Category, XTreamAPI};
use anyhow::Result;
use inquire::Select;
use tracing::{info, warn};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

pub struct MenuSystem {
    providers: Vec<ProviderConfig>,
    current_api: Option<XTreamAPI>,
    player: Player,
    page_size: usize,
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Live,
    Movies,
    Series,
}

#[derive(Debug, Clone)]
pub enum MainMenuOption {
    Content(ContentType),
    RefreshCache,
    ClearCache,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Live => write!(f, "Live TV"),
            ContentType::Movies => write!(f, "Movies (VOD)"),
            ContentType::Series => write!(f, "TV Series"),
        }
    }
}

impl std::fmt::Display for MainMenuOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MainMenuOption::Content(content_type) => write!(f, "{}", content_type),
            MainMenuOption::RefreshCache => write!(f, "Refresh Cache"),
            MainMenuOption::ClearCache => write!(f, "Clear Cache"),
        }
    }
}

impl MenuSystem {
    pub fn new(providers: Vec<ProviderConfig>, player: Player, page_size: usize) -> Self {
        Self {
            providers,
            current_api: None,
            player,
            page_size,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("Welcome to IPTV Rust Player!");

        if self.providers.is_empty() {
            println!("No providers configured. Please check your configuration file.");
            return Ok(());
        }

        // If multiple providers, show provider selection first
        if self.providers.len() > 1 {
            loop {
                if let Some(provider) = self.select_provider().await? {
                    if let Err(e) = self.connect_to_provider(&provider).await {
                        println!("❌ Failed to connect to provider: {}", e);
                        continue;
                    }
                } else {
                    println!("Goodbye!");
                    return Ok(());
                }

                // Check if player is available
                if !self.player.is_available() {
                    println!("Warning: Media player not found. Videos may not play correctly.");
                }

                // Run main menu loop for this provider
                while let Some(menu_option) = self.show_main_menu().await? {
                    match menu_option {
                        MainMenuOption::Content(content_type) => {
                            if let Err(e) = self.browse_content(content_type).await {
                                println!("❌ Error: {}", e);
                                println!("Press Enter to continue...");
                                let _ = std::io::stdin().read_line(&mut String::new());
                            }
                        }
                        MainMenuOption::RefreshCache => {
                            if let Err(e) = self.refresh_cache().await {
                                println!("❌ Error refreshing cache: {}", e);
                                println!("Press Enter to continue...");
                                let _ = std::io::stdin().read_line(&mut String::new());
                            }
                        }
                        MainMenuOption::ClearCache => {
                            if let Err(e) = self.clear_cache().await {
                                println!("❌ Error clearing cache: {}", e);
                                println!("Press Enter to continue...");
                                let _ = std::io::stdin().read_line(&mut String::new());
                            }
                        }
                    }
                }
            }
        } else {
            // Single provider, connect directly
            let provider = self.providers[0].clone();
            self.connect_to_provider(&provider).await?;

            // Check if player is available
            if !self.player.is_available() {
                println!("Warning: Media player not found. Videos may not play correctly.");
            }

            while let Some(menu_option) = self.show_main_menu().await? {
                match menu_option {
                    MainMenuOption::Content(content_type) => {
                        if let Err(e) = self.browse_content(content_type).await {
                            println!("❌ Error: {}", e);
                            println!("Press Enter to continue...");
                            let _ = std::io::stdin().read_line(&mut String::new());
                        }
                    }
                    MainMenuOption::RefreshCache => {
                        if let Err(e) = self.refresh_cache().await {
                            println!("❌ Error refreshing cache: {}", e);
                            println!("Press Enter to continue...");
                            let _ = std::io::stdin().read_line(&mut String::new());
                        }
                    }
                    MainMenuOption::ClearCache => {
                        if let Err(e) = self.clear_cache().await {
                            println!("❌ Error clearing cache: {}", e);
                            println!("Press Enter to continue...");
                            let _ = std::io::stdin().read_line(&mut String::new());
                        }
                    }
                }
            }

            println!("Goodbye!");
        }

        Ok(())
    }

    async fn select_provider(&self) -> Result<Option<ProviderConfig>> {
        let provider_names: Vec<String> = self
            .providers
            .iter()
            .map(|p| {
                p.name.clone().unwrap_or_else(|| {
                    // Extract hostname from URL if no name is provided
                    if let Ok(url) = url::Url::parse(&p.url) {
                        url.host_str().unwrap_or(&p.url).to_string()
                    } else {
                        p.url.clone()
                    }
                })
            })
            .collect();

        let selection = Select::new("Select IPTV provider:", provider_names.clone())
            .with_page_size(self.page_size)
            .prompt_skippable()?;

        if let Some(selected_name) = selection {
            let selected_index = provider_names
                .iter()
                .position(|name| name == &selected_name)
                .unwrap();
            Ok(Some(self.providers[selected_index].clone()))
        } else {
            Ok(None)
        }
    }

    async fn connect_to_provider(&mut self, provider: &ProviderConfig) -> Result<()> {
        info!(
            "Connecting to provider: {}",
            provider.name.as_ref().unwrap_or(&provider.url)
        );

        let mut api = XTreamAPI::new(
            provider.url.clone(),
            provider.username.clone(),
            provider.password.clone(),
            3600, // Default cache TTL
            provider.name.clone(),
        )?;

        // Verify API connection
        match api.get_user_info().await {
            Ok(user_info) => {
                if user_info.auth == 1 {
                    // Parse expiration timestamp and format as human-readable date
                    let expires_msg = if let Ok(exp_timestamp) = user_info.exp_date.parse::<i64>() {
                        let exp_date = DateTime::from_timestamp(exp_timestamp, 0)
                            .unwrap_or_else(Utc::now);
                        format!("Expires: {}", exp_date.format("%Y-%m-%d %H:%M:%S UTC"))
                    } else {
                        format!("Expires: {}", user_info.exp_date)
                    };
                    
                    info!("Connected! {}", expires_msg);

                    // Warm the cache on first connection
                    if let Err(e) = api.warm_cache().await {
                        warn!("Failed to warm cache: {}", e);
                    }

                    self.current_api = Some(api);
                    Ok(())
                } else {
                    println!("Authentication failed: {}", user_info.message);
                    Err(anyhow::anyhow!("Authentication failed"))
                }
            }
            Err(e) => {
                println!("Connection failed: {}", e);
                Err(e)
            }
        }
    }

    async fn show_main_menu(&self) -> Result<Option<MainMenuOption>> {
        let options = vec![
            MainMenuOption::Content(ContentType::Live),
            MainMenuOption::Content(ContentType::Movies),
            MainMenuOption::Content(ContentType::Series),
            MainMenuOption::RefreshCache,
            MainMenuOption::ClearCache,
        ];

        let selection = Select::new("Select option:", options)
            .with_page_size(self.page_size)
            .prompt_skippable()?;

        Ok(selection)
    }

    async fn browse_content(&mut self, content_type: ContentType) -> Result<()> {
        loop {
            // Get categories
            let categories = {
                let api = self
                    .current_api
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
                match content_type {
                    ContentType::Live => api.get_live_categories().await?,
                    ContentType::Movies => api.get_vod_categories().await?,
                    ContentType::Series => api.get_series_categories().await?,
                }
            };

            let category = self.select_category(&categories).await?;

            match category {
                Some(cat) => {
                    let category_id = if cat.category_id == "all" {
                        None
                    } else {
                        Some(cat.category_id.as_str())
                    };

                    let result = match content_type {
                        ContentType::Live => self.browse_streams(category_id, "live").await,
                        ContentType::Movies => self.browse_streams(category_id, "movie").await,
                        ContentType::Series => self.browse_series_list(category_id).await,
                    };

                    if let Err(e) = result {
                        println!("Error loading content: {}", e);
                    }
                }
                None => break, // Go back
            }
        }
        Ok(())
    }

    async fn select_category(&self, categories: &[Category]) -> Result<Option<Category>> {
        let mut options = vec![Category {
            category_id: "all".to_string(),
            category_name: "All".to_string(),
            parent_id: None,
        }];

        options.extend(
            categories
                .iter()
                .map(|cat| Category {
                    category_id: cat.category_id.clone(),
                    category_name: cat.category_name.clone(),
                    parent_id: cat.parent_id,
                })
                .collect::<Vec<_>>(),
        );

        let selection = Select::new("Select category:", options)
            .with_page_size(self.page_size)
            .prompt_skippable()?;

        Ok(selection)
    }

    async fn browse_streams(&mut self, category_id: Option<&str>, stream_type: &str) -> Result<()> {
        let streams = {
            let api = self
                .current_api
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
            match stream_type {
                "live" => api.get_live_streams(category_id).await?,
                "movie" => api.get_vod_streams(category_id).await?,
                _ => return Ok(()),
            }
        };

        if streams.is_empty() {
            println!("No streams found in this category.");
            return Ok(());
        }

        // Create stream display options and maintain mapping for de-duplicated movies
        let (stream_options, display_to_stream_map): (Vec<String>, HashMap<String, usize>) = 
            if category_id.is_none() || category_id == Some("all") {
                // For "All" category, include category names in brackets
                let category_map = match stream_type {
                    "live" => {
                        let api = self
                            .current_api
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
                        let categories = api.get_live_categories().await?;
                        categories.into_iter()
                            .map(|cat| (cat.category_id, cat.category_name))
                            .collect::<HashMap<String, String>>()
                    }
                    "movie" => {
                        let api = self
                            .current_api
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
                        let categories = api.get_vod_categories().await?;
                        categories.into_iter()
                            .map(|cat| (cat.category_id, cat.category_name))
                            .collect::<HashMap<String, String>>()
                    }
                    _ => HashMap::new(),
                };
                
                if stream_type == "movie" {
                    // For movies, de-duplicate by stream_id and collect all categories
                    let mut movie_map: HashMap<u32, (String, Vec<String>, usize)> = HashMap::new();
                    
                    for (index, stream) in streams.iter().enumerate() {
                        let category_name = category_map.get(&stream.category_id)
                            .cloned()
                            .unwrap_or_else(|| "Unknown".to_string());
                        
                        movie_map.entry(stream.stream_id)
                            .and_modify(|(_, categories, _)| categories.push(category_name.clone()))
                            .or_insert_with(|| (stream.name.clone(), vec![category_name], index));
                    }
                    
                    let mut options = Vec::new();
                    let mut mapping = HashMap::new();
                    
                    for (name, categories, first_index) in movie_map.values() {
                        let display_name = if categories.is_empty() {
                            name.clone()
                        } else {
                            format!("{} [{}]", name, categories.join(", "))
                        };
                        mapping.insert(display_name.clone(), *first_index);
                        options.push(display_name);
                    }
                    
                    (options, mapping)
                } else {
                    // For live streams, show individual streams with their category
                    let options: Vec<String> = streams.iter().map(|stream| {
                        if let Some(category_name) = category_map.get(&stream.category_id) {
                            format!("{} [{}]", stream.name, category_name)
                        } else {
                            stream.name.clone()
                        }
                    }).collect();
                    
                    // Create 1:1 mapping for non-deduplicated streams
                    let mapping = options.iter().enumerate()
                        .map(|(index, name)| (name.clone(), index))
                        .collect();
                    
                    (options, mapping)
                }
            } else {
                // For specific categories, just show stream names
                let options: Vec<String> = streams.iter().map(|stream| stream.name.clone()).collect();
                let mapping = options.iter().enumerate()
                    .map(|(index, name)| (name.clone(), index))
                    .collect();
                (options, mapping)
            };

        if stream_options.is_empty() {
            println!("No streams available.");
            return Ok(());
        }

        let mut last_selected_index = 0;

        loop {
            let mut select = Select::new("Select stream to play:", stream_options.clone())
                .with_page_size(self.page_size);

            // Set the cursor to the last selected item
            select = select.with_starting_cursor(last_selected_index);

            let selection = select.prompt_skippable()?;

            if let Some(selected_name) = selection {
                // Find the selected stream using the mapping
                let display_index = stream_options
                    .iter()
                    .position(|opt| opt == &selected_name)
                    .unwrap();

                // Remember this selection for next time
                last_selected_index = display_index;

                // Get the actual stream index from the mapping
                let stream_index = display_to_stream_map
                    .get(&selected_name)
                    .copied()
                    .unwrap_or(display_index);

                let selected_stream = &streams[stream_index];
                let url = {
                    let api = self
                        .current_api
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
                    api.get_stream_url(selected_stream.stream_id, stream_type, None)
                };

                println!("Playing: {}", selected_stream.name);
                if let Err(e) = self.player.play(&url) {
                    println!("Playback error: {}", e);
                }
            } else {
                break; // Go back
            }
        }

        Ok(())
    }

    async fn browse_series_list(&mut self, category_id: Option<&str>) -> Result<()> {
        let series = {
            let api = self
                .current_api
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
            api.get_series(category_id).await?
        };

        if series.is_empty() {
            println!("No series found in this category.");
            return Ok(());
        }

        let series_options: Vec<String> = series.iter().map(|s| s.name.clone()).collect();

        let mut last_selected_index = 0;

        loop {
            let mut select = Select::new("Select series:", series_options.clone())
                .with_page_size(self.page_size);

            // Set the cursor to the last selected item
            select = select.with_starting_cursor(last_selected_index);

            let selection = select.prompt_skippable()?;

            if let Some(selected_name) = selection {
                let selected_index = series_options
                    .iter()
                    .position(|opt| opt == &selected_name)
                    .unwrap();

                // Remember this selection for next time
                last_selected_index = selected_index;

                let selected_series = &series[selected_index];

                // For series, we would need to fetch episodes here
                // This is a simplified version that shows series info
                println!("Series: {}", selected_series.name);
                if let Some(ref plot) = selected_series.plot {
                    println!("Plot: {}", plot);
                }
                if let Some(ref genre) = selected_series.genre {
                    println!("Genre: {}", genre);
                }
                if let Some(ref release_date) = selected_series.release_date {
                    println!("Release: {}", release_date);
                }

                println!("Episode browsing not yet implemented.");
            } else {
                break;
            }
        }

        Ok(())
    }

    async fn refresh_cache(&mut self) -> Result<()> {
        println!("Refreshing cache...");

        if let Some(ref mut api) = self.current_api {
            api.clear_cache().await?;
            api.warm_cache().await?;
            println!("Cache refreshed successfully!");
        } else {
            println!("No provider connected");
        }

        println!("Press Enter to continue...");
        let _ = std::io::stdin().read_line(&mut String::new());
        Ok(())
    }

    async fn clear_cache(&mut self) -> Result<()> {
        println!("Clearing cache...");

        if let Some(ref mut api) = self.current_api {
            api.clear_cache().await?;
            println!("Cache cleared successfully!");
        } else {
            println!("No provider connected");
        }

        println!("Press Enter to continue...");
        let _ = std::io::stdin().read_line(&mut String::new());
        Ok(())
    }
}
