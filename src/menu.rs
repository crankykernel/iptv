// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use inquire::Select;
use crate::xtream_api::{XTreamAPI, Category, Stream, SeriesInfo};
use crate::player::Player;

pub struct MenuSystem {
    api: XTreamAPI,
    player: Player,
    page_size: usize,
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Live,
    Movies,
    Series,
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

impl MenuSystem {
    pub fn new(api: XTreamAPI, player: Player, page_size: usize) -> Self {
        Self {
            api,
            player,
            page_size,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("Welcome to IPTV Rust Player!");
        
        // Verify API connection
        match self.api.get_user_info().await {
            Ok(user_info) => {
                if user_info.auth == 1 {
                    println!("Connected as: {}", user_info.username);
                    println!("Expires: {}", user_info.exp_date);
                } else {
                    println!("Authentication failed: {}", user_info.message);
                    return Ok(());
                }
            }
            Err(e) => {
                println!("Connection failed: {}", e);
                return Ok(());
            }
        }

        // Check if player is available
        if !self.player.is_available() {
            println!("Warning: Media player not found. Videos may not play correctly.");
        }

        loop {
            match self.show_main_menu().await? {
                Some(content_type) => {
                    if let Err(e) = self.browse_content(content_type).await {
                        println!("❌ Error: {}", e);
                        println!("Press Enter to continue...");
                        let _ = std::io::stdin().read_line(&mut String::new());
                    }
                }
                None => break,
            }
        }

        println!("Goodbye!");
        Ok(())
    }

    async fn show_main_menu(&self) -> Result<Option<ContentType>> {
        let options = vec![
            ContentType::Live,
            ContentType::Movies, 
            ContentType::Series,
        ];

        let selection = Select::new("Select content type:", options)
            .with_page_size(self.page_size)
            .prompt_skippable()?;

        Ok(selection)
    }

    async fn browse_content(&mut self, content_type: ContentType) -> Result<()> {
        loop {
            // Get categories
            let categories = match content_type {
                ContentType::Live => self.api.get_live_categories().await?,
                ContentType::Movies => self.api.get_vod_categories().await?,
                ContentType::Series => self.api.get_series_categories().await?,
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
                .collect::<Vec<_>>()
        );

        let selection = Select::new("Select category:", options)
            .with_page_size(self.page_size)
            .prompt_skippable()?;

        Ok(selection)
    }

    async fn browse_streams(&mut self, category_id: Option<&str>, stream_type: &str) -> Result<()> {
        let streams = match stream_type {
            "live" => self.api.get_live_streams(category_id).await?,
            "movie" => self.api.get_vod_streams(category_id).await?,
            _ => return Ok(()),
        };

        if streams.is_empty() {
            println!("No streams found in this category.");
            return Ok(());
        }

        let stream_options: Vec<String> = streams
            .iter()
            .map(|stream| stream.name.clone())
            .collect();

        if stream_options.is_empty() {
            println!("No streams available.");
            return Ok(());
        }

        loop {
            let selection = Select::new("Select stream to play:", stream_options.clone())
                .with_page_size(self.page_size)
                .prompt_skippable()?;

            if let Some(selected_name) = selection {
                // Find the selected stream
                let selected_index = stream_options
                    .iter()
                    .position(|opt| opt == &selected_name)
                    .unwrap();
                
                let selected_stream = &streams[selected_index];
                let url = self.api.get_stream_url(
                    selected_stream.stream_id,
                    stream_type,
                    None,
                );

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
        let series = self.api.get_series(category_id).await?;

        if series.is_empty() {
            println!("No series found in this category.");
            return Ok(());
        }

        let series_options: Vec<String> = series
            .iter()
            .map(|s| s.name.clone())
            .collect();

        loop {
            let selection = Select::new("Select series:", series_options.clone())
                .with_page_size(self.page_size)
                .prompt_skippable()?;

            if let Some(selected_name) = selection {
                let selected_index = series_options
                    .iter()
                    .position(|opt| opt == &selected_name)
                    .unwrap();
                
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
}