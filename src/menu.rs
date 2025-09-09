// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::config::ProviderConfig;
use crate::player::Player;
use crate::xtream_api::{Category, Episode, Season, XTreamAPI};
use crate::FavouritesManager;
use anyhow::Result;
use chrono::{DateTime, Utc};
use inquire::Select;
use std::collections::HashMap;
use tracing::{debug, info, warn};

enum ProviderSelection {
    Provider(ProviderConfig),
    AllFavourites,
    Exit,
}

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
    Favourites,
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
            MainMenuOption::Favourites => write!(f, "🌟 Favourites"),
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
                match self.select_provider_or_favourites().await? {
                    ProviderSelection::Provider(provider) => {
                        if let Err(e) = self.connect_to_provider(&provider).await {
                            println!("❌ Failed to connect to provider: {}", e);
                            continue;
                        }
                    }
                    ProviderSelection::AllFavourites => {
                        if let Err(e) = self.browse_all_favourites().await {
                            println!("❌ Error: {}", e);
                            println!("Press Enter to continue...");
                            let _ = std::io::stdin().read_line(&mut String::new());
                        }
                        continue;
                    }
                    ProviderSelection::Exit => {
                        println!("Goodbye!");
                        return Ok(());
                    }
                }

                // Check if player is available
                if !self.player.is_available() {
                    println!("Warning: Media player not found. Videos may not play correctly.");
                }

                // Run main menu loop for this provider
                while let Some(menu_option) = self.show_main_menu().await? {
                    match menu_option {
                        MainMenuOption::Favourites => {
                            if let Err(e) = self.browse_favourites().await {
                                println!("❌ Error: {}", e);
                                println!("Press Enter to continue...");
                                let _ = std::io::stdin().read_line(&mut String::new());
                            }
                        }
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
                    MainMenuOption::Favourites => {
                        if let Err(e) = self.browse_favourites().await {
                            println!("❌ Error: {}", e);
                            println!("Press Enter to continue...");
                            let _ = std::io::stdin().read_line(&mut String::new());
                        }
                    }
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

    async fn select_provider_or_favourites(&self) -> Result<ProviderSelection> {
        // Build menu options
        let mut options = vec!["⭐ All Favourites".to_string()];
        
        // Add provider names
        for provider in &self.providers {
            let name = provider.name.clone().unwrap_or_else(|| {
                // Extract hostname from URL if no name is provided
                if let Ok(url) = url::Url::parse(&provider.url) {
                    url.host_str().unwrap_or(&provider.url).to_string()
                } else {
                    provider.url.clone()
                }
            });
            options.push(format!("📡 {}", name));
        }

        let selection = Select::new("Select an option:", options.clone())
            .with_page_size(self.page_size)
            .prompt_skippable()?;

        match selection {
            Some(selected) if selected == "⭐ All Favourites" => {
                Ok(ProviderSelection::AllFavourites)
            }
            Some(selected) => {
                // Find the provider index (subtract 1 for the "All Favourites" option)
                let provider_index = options.iter()
                    .position(|opt| opt == &selected)
                    .unwrap() - 1;
                Ok(ProviderSelection::Provider(self.providers[provider_index].clone()))
            }
            None => Ok(ProviderSelection::Exit),
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
            provider.name.clone(),
        )?;

        // Verify API connection
        match api.get_user_info().await {
            Ok(user_info) => {
                if user_info.auth == 1 {
                    // Parse expiration timestamp and format as human-readable date
                    let expires_msg = if let Ok(exp_timestamp) = user_info.exp_date.parse::<i64>() {
                        let exp_date =
                            DateTime::from_timestamp(exp_timestamp, 0).unwrap_or_else(Utc::now);
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
            MainMenuOption::Favourites,
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

    async fn browse_all_favourites(&mut self) -> Result<()> {
        println!("\n⭐ All Favourites (across all providers)");
        println!("===========================================");
        
        let favourites_manager = FavouritesManager::new()?;
        let mut all_favourites = Vec::new();
        
        // Collect favourites from all providers
        for provider in &self.providers {
            let api = XTreamAPI::new(
                provider.url.clone(),
                provider.username.clone(),
                provider.password.clone(),
                provider.name.clone(),
            )?;
            
            let provider_favs = favourites_manager.get_favourites(&api.provider_hash)?;
            for fav in provider_favs {
                all_favourites.push((fav, provider.clone()));
            }
        }
        
        if all_favourites.is_empty() {
            println!("No favourites found across any provider.");
            println!("Press Enter to continue...");
            let _ = std::io::stdin().read_line(&mut String::new());
            return Ok(());
        }
        
        loop {
            let favourite_options: Vec<String> = all_favourites
                .iter()
                .map(|(fav, provider)| {
                    let provider_name = provider.name.as_ref()
                        .unwrap_or(&provider.url);
                    format!("{} [{}]", fav.name, provider_name)
                })
                .collect();
            
            let selection = Select::new("Select a favourite:", favourite_options.clone())
                .with_page_size(self.page_size)
                .prompt_skippable()?;
            
            if let Some(selected_name) = selection {
                let selected_index = favourite_options
                    .iter()
                    .position(|name| name == &selected_name)
                    .unwrap();
                
                let (selected_favourite, provider) = &all_favourites[selected_index];
                
                // Connect to the provider if not already connected
                if self.current_api.is_none() || 
                   self.current_api.as_ref().unwrap().provider_hash != 
                   XTreamAPI::new(provider.url.clone(), provider.username.clone(), 
                                  provider.password.clone(), provider.name.clone())?.provider_hash {
                    self.connect_to_provider(&provider).await?;
                }
                
                // Play the favourite
                let api = self.current_api.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
                let stream_url = api.get_stream_url(
                    selected_favourite.stream_id,
                    &selected_favourite.stream_type,
                    None,
                );
                self.player.play(&stream_url)?;
            } else {
                break;
            }
        }
        
        Ok(())
    }

    async fn browse_favourites(&mut self) -> Result<()> {
        let mut last_selected_index = 0;

        loop {
            let api = self
                .current_api
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

            let favourites = api.favourites_manager.get_favourites(&api.provider_hash)?;

            if favourites.is_empty() {
                println!("No favourites yet. Browse Live TV to add some!");
                println!("Press Enter to continue...");
                let _ = std::io::stdin().read_line(&mut String::new());
                return Ok(());
            }

            let favourite_options: Vec<String> = favourites
                .iter()
                .map(|fav| format!("⭐ {}", fav.name))
                .collect();

            // Adjust cursor if it's out of bounds after deletion
            if last_selected_index >= favourite_options.len() {
                last_selected_index = favourite_options.len().saturating_sub(1);
            }

            let mut select = Select::new(
                "Select favourite to play or manage:",
                favourite_options.clone(),
            )
            .with_page_size(self.page_size);

            select = select.with_starting_cursor(last_selected_index);
            let selection = select.prompt_skippable()?;

            if let Some(selected_name) = selection {
                let selected_index = favourite_options
                    .iter()
                    .position(|opt| opt == &selected_name)
                    .unwrap();

                last_selected_index = selected_index;
                let selected_favourite = &favourites[selected_index];

                // Show action menu
                let actions = vec!["▶ Play Stream", "🗑 Remove from Favourites"];
                let action_selection = Select::new(
                    &format!("Action for '{}':", selected_favourite.name),
                    actions,
                )
                .prompt_skippable()?;

                match action_selection {
                    Some("▶ Play Stream") => {
                        let url = api.get_stream_url(
                            selected_favourite.stream_id,
                            &selected_favourite.stream_type,
                            None,
                        );
                        println!("Playing: {}", selected_favourite.name);
                        if let Err(e) = self.player.play(&url) {
                            println!("Playback error: {}", e);
                        }
                    }
                    Some("🗑 Remove from Favourites") => {
                        let api_mut = self
                            .current_api
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

                        api_mut.favourites_manager.remove_favourite(
                            &api_mut.provider_hash,
                            selected_favourite.stream_id,
                            &selected_favourite.stream_type,
                        )?;

                        println!("Removed '{}' from favourites", selected_favourite.name);
                        // Continue loop to reload favourites
                    }
                    _ => {} // Back/Cancel
                }
            } else {
                break; // Go back
            }
        }

        Ok(())
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

        // Get all favourites for live streams to show indicators
        let favourites = if stream_type == "live" {
            let api = self
                .current_api
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
            api.favourites_manager
                .get_favourites(&api.provider_hash)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let favourite_stream_ids: std::collections::HashSet<u32> = favourites
            .iter()
            .filter(|f| f.stream_type == stream_type)
            .map(|f| f.stream_id)
            .collect();

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
                        categories
                            .into_iter()
                            .map(|cat| (cat.category_id, cat.category_name))
                            .collect::<HashMap<String, String>>()
                    }
                    "movie" => {
                        let api = self
                            .current_api
                            .as_mut()
                            .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;
                        let categories = api.get_vod_categories().await?;
                        categories
                            .into_iter()
                            .map(|cat| (cat.category_id, cat.category_name))
                            .collect::<HashMap<String, String>>()
                    }
                    _ => HashMap::new(),
                };

                if stream_type == "movie" {
                    // For movies, de-duplicate by stream_id and collect all categories
                    let mut movie_map: HashMap<u32, (String, Vec<String>, usize)> = HashMap::new();

                    for (index, stream) in streams.iter().enumerate() {
                        let category_name = stream
                            .category_id
                            .as_ref()
                            .and_then(|id| category_map.get(id).cloned())
                            .unwrap_or_else(|| "Unknown".to_string());

                        movie_map
                            .entry(stream.stream_id)
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
                    // For live streams, show individual streams with their category and favourite indicator
                    let options: Vec<String> = streams
                        .iter()
                        .map(|stream| {
                            let fav_indicator = if favourite_stream_ids.contains(&stream.stream_id)
                            {
                                "⭐ "
                            } else {
                                ""
                            };

                            if let Some(category_name) = stream
                                .category_id
                                .as_ref()
                                .and_then(|id| category_map.get(id))
                            {
                                format!("{}{} [{}]", fav_indicator, stream.name, category_name)
                            } else {
                                format!("{}{}", fav_indicator, stream.name)
                            }
                        })
                        .collect();

                    // Create 1:1 mapping for non-deduplicated streams
                    let mapping = options
                        .iter()
                        .enumerate()
                        .map(|(index, name)| (name.clone(), index))
                        .collect();

                    (options, mapping)
                }
            } else {
                // For specific categories, show stream names with favourite indicator
                let options: Vec<String> = streams
                    .iter()
                    .map(|stream| {
                        let fav_indicator = if favourite_stream_ids.contains(&stream.stream_id) {
                            "⭐ "
                        } else {
                            ""
                        };
                        format!("{}{}", fav_indicator, stream.name)
                    })
                    .collect();
                let mapping = options
                    .iter()
                    .enumerate()
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

                if stream_type == "movie" {
                    // For movies, show info directly without the action menu
                    match self
                        .handle_movie_playback(selected_stream.stream_id, &selected_stream.name)
                        .await
                    {
                        Ok(_) => {}
                        Err(e) => {
                            println!("Movie playback error: {}", e);
                        }
                    }
                } else {
                    // For live streams, show the action menu
                    let api = self
                        .current_api
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

                    let is_fav = api.favourites_manager.is_favourite(
                        &api.provider_hash,
                        selected_stream.stream_id,
                        stream_type,
                    )?;

                    // Show action menu
                    let mut actions = vec!["▶ Play Stream"];
                    if stream_type == "live" {
                        // Only allow favourites for live streams for now
                        if is_fav {
                            actions.push("🗑 Remove from Favourites");
                        } else {
                            actions.push("⭐ Add to Favourites");
                        }
                    }

                    let action_selection =
                        Select::new(&format!("Action for '{}':", selected_stream.name), actions)
                            .prompt_skippable()?;

                    match action_selection {
                        Some("▶ Play Stream") => {
                            let url =
                                api.get_stream_url(selected_stream.stream_id, stream_type, None);
                            println!("Playing: {}", selected_stream.name);
                            if let Err(e) = self.player.play(&url) {
                                println!("Playback error: {}", e);
                            }
                        }
                        Some("⭐ Add to Favourites") => {
                            let api_mut = self
                                .current_api
                                .as_mut()
                                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

                            use crate::xtream_api::FavouriteStream;
                            use chrono::Utc;

                            let favourite = FavouriteStream {
                                stream_id: selected_stream.stream_id,
                                name: selected_stream.name.clone(),
                                stream_type: stream_type.to_string(),
                                provider_hash: api_mut.provider_hash.clone(),
                                added_date: Utc::now(),
                                category_id: selected_stream.category_id.clone(),
                            };

                            api_mut
                                .favourites_manager
                                .add_favourite(&api_mut.provider_hash, favourite)?;

                            println!("Added '{}' to favourites!", selected_stream.name);
                        }
                        Some("🗑 Remove from Favourites") => {
                            let api_mut = self
                                .current_api
                                .as_mut()
                                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

                            api_mut.favourites_manager.remove_favourite(
                                &api_mut.provider_hash,
                                selected_stream.stream_id,
                                stream_type,
                            )?;

                            println!("Removed '{}' from favourites", selected_stream.name);
                        }
                        _ => {} // Back/Cancel
                    }
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

                // Fetch detailed series information with episodes
                println!("Loading episodes for: {}", selected_series.name);
                match self.browse_episodes(selected_series.series_id).await {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Failed to load episodes: {}", e);
                        println!("Press Enter to continue...");
                        std::io::stdin().read_line(&mut String::new()).ok();
                    }
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    async fn browse_episodes(&mut self, series_id: u32) -> Result<()> {
        let api = self
            .current_api
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

        // Get detailed series info with episodes
        let series_info = api.get_series_info(series_id).await?;

        debug!(
            "Series info parsed: {} seasons, episodes present: {}",
            series_info.seasons.len(),
            series_info.episodes.is_some()
        );

        // Display series info
        let series_name = if let Some(ref info) = series_info.info {
            &info.name
        } else {
            "Unknown Series"
        };
        println!("\n=== {} ===", series_name);
        if let Some(ref info) = series_info.info {
            if let Some(ref plot) = info.plot {
                println!("Plot: {}", plot);
            }
            if let Some(ref genre) = info.genre {
                println!("Genre: {}", genre);
            }
            if let Some(ref release_date) = info.release_date {
                println!("Release: {}", release_date);
            }
            if let Some(ref rating) = info.rating {
                println!("Rating: {}", rating);
            }
        }
        println!();

        if series_info.seasons.is_empty() {
            println!("No seasons found for this series.");
            println!("Press Enter to continue...");
            std::io::stdin().read_line(&mut String::new()).ok();
            return Ok(());
        }

        // Browse seasons and episodes
        let mut last_selected_season = 0;
        loop {
            // Create season options from API response
            let season_options: Vec<String> = series_info
                .seasons
                .iter()
                .map(|season| {
                    let episode_count = season.episode_count.parse::<u32>().unwrap_or(0);
                    format!(
                        "Season {} - {} ({} episodes)",
                        season.season_number, season.name, episode_count
                    )
                })
                .collect();

            let mut season_options_with_back = season_options.clone();
            season_options_with_back.push("⬅ Back to Series".to_string());

            // Ensure valid cursor position
            if last_selected_season >= season_options.len() {
                last_selected_season = season_options.len().saturating_sub(1);
            }

            let select = Select::new("Select Season:", season_options_with_back)
                .with_page_size(self.page_size)
                .with_starting_cursor(last_selected_season);

            match select.prompt_skippable()? {
                Some(selection) => {
                    if selection == "⬅ Back to Series" {
                        break;
                    }

                    // Find selected season index
                    if let Some(season_index) =
                        season_options.iter().position(|opt| *opt == selection)
                    {
                        last_selected_season = season_index;
                        let selected_api_season = &series_info.seasons[season_index];

                        // Convert API season to internal format
                        let episodes = if let Some(ref episodes_map) = series_info.episodes {
                            // Try to get episodes by season number
                            let season_key = &selected_api_season.season_number.to_string();
                            debug!("Looking for episodes in season key: {}", season_key);
                            debug!(
                                "Available episode keys: {:?}",
                                episodes_map.keys().collect::<Vec<_>>()
                            );

                            episodes_map
                                .get(season_key)
                                .unwrap_or(&Vec::new())
                                .iter()
                                .map(|api_ep| Episode {
                                    id: api_ep.id.clone(),
                                    episode_num: api_ep.episode_num,
                                    title: api_ep.title.clone(),
                                    container_extension: api_ep.container_extension.clone(),
                                    info: api_ep.info.clone(),
                                    custom_sid: api_ep.custom_sid.clone(),
                                    added: api_ep.added.clone(),
                                    season: api_ep.season,
                                    direct_source: api_ep.direct_source.clone(),
                                })
                                .collect()
                        } else {
                            debug!("No episodes map in response, might need separate API call");
                            Vec::new()
                        };

                        let season = Season {
                            season_number: selected_api_season.season_number,
                            name: Some(selected_api_season.name.clone()),
                            episodes,
                        };

                        // Browse episodes in this season
                        self.browse_season_episodes(&season, series_name).await?;
                    }
                }
                None => break,
            }
        }

        Ok(())
    }

    async fn browse_season_episodes(
        &mut self,
        season: &crate::xtream_api::Season,
        series_name: &str,
    ) -> Result<()> {
        let mut last_selected_episode = 0;

        loop {
            // Create episode options
            let episode_options: Vec<String> = season
                .episodes
                .iter()
                .map(|episode| {
                    let duration_info = if let Some(ref info) = episode.info {
                        if let Some(ref duration) = info.duration {
                            format!(" ({})", duration)
                        } else if let Some(duration_secs) = info.duration_secs {
                            let minutes = duration_secs / 60;
                            let seconds = duration_secs % 60;
                            format!(" ({}:{:02})", minutes, seconds)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    format!(
                        "Episode {} - {}{}",
                        episode.episode_num, episode.title, duration_info
                    )
                })
                .collect();

            if episode_options.is_empty() {
                println!("No episodes found in this season.");
                println!("Press Enter to continue...");
                std::io::stdin().read_line(&mut String::new()).ok();
                return Ok(());
            }

            let mut episode_options_with_back = episode_options.clone();
            episode_options_with_back.push("⬅ Back to Seasons".to_string());

            // Ensure valid cursor position
            if last_selected_episode >= episode_options.len() {
                last_selected_episode = episode_options.len().saturating_sub(1);
            }

            let season_name = if let Some(ref name) = season.name {
                format!("Season {} - {}", season.season_number, name)
            } else {
                format!("Season {}", season.season_number)
            };

            let prompt_text = format!("{} - {} Episodes:", series_name, season_name);
            let select = Select::new(&prompt_text, episode_options_with_back)
                .with_page_size(self.page_size)
                .with_starting_cursor(last_selected_episode);

            match select.prompt_skippable()? {
                Some(selection) => {
                    if selection == "⬅ Back to Seasons" {
                        break;
                    }

                    // Find selected episode index
                    if let Some(episode_index) =
                        episode_options.iter().position(|opt| *opt == selection)
                    {
                        last_selected_episode = episode_index;
                        let selected_episode = &season.episodes[episode_index];

                        // Show episode details and play option
                        self.handle_episode_selection(selected_episode, series_name)
                            .await?;
                    }
                }
                None => break,
            }
        }

        Ok(())
    }

    async fn handle_episode_selection(
        &mut self,
        episode: &crate::xtream_api::Episode,
        series_name: &str,
    ) -> Result<()> {
        // Display episode details
        println!(
            "\n=== {} - Episode {} ===",
            series_name, episode.episode_num
        );
        println!("Title: {}", episode.title);

        if let Some(ref info) = episode.info {
            if let Some(ref plot) = info.plot {
                println!("Plot: {}", plot);
            }
            if let Some(ref release_date) = info.releasedate {
                println!("Release Date: {}", release_date);
            }
            if let Some(ref rating) = info.rating {
                println!("Rating: {}", rating);
            }
            if let Some(ref duration) = info.duration {
                println!("Duration: {}", duration);
            } else if let Some(duration_secs) = info.duration_secs {
                let minutes = duration_secs / 60;
                let seconds = duration_secs % 60;
                println!("Duration: {}:{:02}", minutes, seconds);
            }
        }

        // Episode action menu
        let actions = vec!["▶ Play Episode", "⬅ Back"];
        let action_selection = Select::new(
            &format!("Action for Episode {}:", episode.episode_num),
            actions,
        )
        .prompt_skippable()?;

        if let Some("▶ Play Episode") = action_selection {
            let api = self
                .current_api
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

            let stream_url =
                api.get_episode_stream_url(&episode.id, episode.container_extension.as_deref());
            println!("Playing: {} - Episode {}", series_name, episode.episode_num);

            self.player.play(&stream_url)?;
        }
        // Back - do nothing

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

    async fn handle_movie_playback(&mut self, stream_id: u32, stream_name: &str) -> Result<()> {
        let api = self
            .current_api
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("No provider connected"))?;

        // Get VOD info for the movie
        println!("Loading movie information...");
        let vod_info = match api.get_vod_info(stream_id).await {
            Ok(info) => info,
            Err(e) => {
                warn!("Failed to get VOD info for stream {}: {}", stream_id, e);
                // Fallback to basic streaming without VOD info
                let url = api.get_stream_url(stream_id, "movie", None);
                println!("Playing: {}", stream_name);
                return self
                    .player
                    .play(&url)
                    .map_err(|e| anyhow::anyhow!("Playback error: {}", e));
            }
        };

        // Get terminal width for text wrapping
        let terminal_width = terminal_size::terminal_size()
            .map(|(w, _)| w.0 as usize)
            .unwrap_or(80);

        // Display movie information
        println!("\n=== {} ===", vod_info.info.name);

        if let Some(ref plot) = vod_info.info.plot {
            println!("Description:");
            Self::print_wrapped(plot, terminal_width);
            println!();
        }

        if let Some(ref genre) = vod_info.info.genre {
            println!("Genre: {}", genre);
        }

        if let Some(ref release_date) = vod_info.info.releasedate {
            println!("Release Date: {}", release_date);
        }

        if let Some(ref duration_value) = vod_info.info.duration_secs {
            // Try to parse duration_secs from various formats
            let duration_opt = match duration_value {
                serde_json::Value::Number(n) => n.as_u64().map(|v| v as u32),
                serde_json::Value::String(s) => s.parse::<u32>().ok(),
                _ => None,
            };

            if let Some(duration) = duration_opt {
                let hours = duration / 3600;
                let minutes = (duration % 3600) / 60;
                if hours > 0 {
                    println!("Duration: {}h {}m", hours, minutes);
                } else {
                    println!("Duration: {}m", minutes);
                }
            } else if let Some(ref duration) = vod_info.info.duration {
                println!("Duration: {}", duration);
            }
        } else if let Some(ref duration) = vod_info.info.duration {
            println!("Duration: {}", duration);
        }

        println!();

        // Show play confirmation
        let actions = vec!["▶ Play Movie", "⬅ Back"];
        let action_selection =
            Select::new(&format!("Action for '{}':", vod_info.info.name), actions)
                .prompt_skippable()?;

        if let Some("▶ Play Movie") = action_selection {
            // Use the container extension from VOD info
            let extension = Some(vod_info.movie_data.container_extension.as_str());
            let url = api.get_stream_url(stream_id, "movie", extension);

            println!(
                "Playing: {} ({})",
                vod_info.info.name, vod_info.movie_data.container_extension
            );
            self.player
                .play(&url)
                .map_err(|e| anyhow::anyhow!("Playback error: {}", e))?;
        }

        Ok(())
    }

    fn print_wrapped(text: &str, width: usize) {
        let indent = "  ";
        let effective_width = width.saturating_sub(indent.len());

        let words: Vec<&str> = text.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= effective_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                println!("{}{}", indent, current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            println!("{}{}", indent, current_line);
        }
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
