// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::config::ProviderConfig;
use crate::menu::ContentType;
use crate::player::Player;
use crate::xtream_api::{ApiEpisode, Category, FavouriteStream, Stream, XTreamAPI};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct TuiSeason {
    pub season_number: u32,
    pub name: String,
    pub episode_count: usize,
}

#[derive(Debug, Clone)]
pub enum AppState {
    ProviderSelection,
    MainMenu,
    CategorySelection(ContentType),
    StreamSelection(ContentType, Category),
    SeasonSelection(Stream),
    EpisodeSelection(Stream, TuiSeason),
    FavouriteSelection,
    Loading(String),
    Error(String),
    Playing(String),
}

#[derive(Debug, Clone)]
pub enum Action {
    Quit,
    Back,
    Select,
    Refresh,
}

pub struct App {
    pub state: AppState,
    pub providers: Vec<ProviderConfig>,
    pub current_api: Option<XTreamAPI>,
    pub player: Player,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub items: Vec<String>,
    pub status_message: Option<String>,
    pub progress: Option<(f64, String)>,
    pub logs: Vec<(Instant, String)>,
    pub show_help: bool,
    pub page_size: usize,
    categories: Vec<Category>,
    streams: Vec<Stream>,
    seasons: Vec<TuiSeason>,
    episodes: Vec<ApiEpisode>,
    favourites: Vec<FavouriteStream>,
}

impl App {
    pub fn new(providers: Vec<ProviderConfig>, player: Player) -> Self {
        let items = if providers.len() > 1 {
            providers
                .iter()
                .map(|p| p.name.clone().unwrap_or_else(|| p.url.clone()))
                .collect()
        } else {
            vec![]
        };

        let state = if providers.len() > 1 {
            AppState::ProviderSelection
        } else if providers.len() == 1 {
            AppState::Loading("Connecting to provider...".to_string())
        } else {
            AppState::Error("No providers configured".to_string())
        };

        Self {
            state,
            providers,
            current_api: None,
            player,
            selected_index: 0,
            scroll_offset: 0,
            items,
            status_message: None,
            progress: None,
            logs: Vec::new(),
            show_help: false,
            page_size: 20,
            categories: Vec::new(),
            streams: Vec::new(),
            seasons: Vec::new(),
            episodes: Vec::new(),
            favourites: Vec::new(),
        }
    }

    pub fn tick(&mut self) {
        // Update any time-based UI elements here
        // Removed the player check as it was spawning tasks unnecessarily
        // Player status will be checked when user presses a key
    }

    pub async fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(Action::Quit);
        }

        if key.code == KeyCode::Char('q') {
            return Some(Action::Quit);
        }

        if key.code == KeyCode::Char('?') || key.code == KeyCode::F(1) {
            self.show_help = !self.show_help;
            return None;
        }

        match &self.state {
            AppState::Error(_) => {
                if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                    self.state = AppState::MainMenu;
                    self.update_main_menu_items();
                }
            }
            AppState::ProviderSelection => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home => self.move_selection_home(),
                KeyCode::End => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index < self.providers.len() {
                        let provider = self.providers[self.selected_index].clone();
                        self.connect_to_provider(&provider).await;
                    }
                }
                KeyCode::Esc => return Some(Action::Quit),
                _ => {}
            },
            AppState::MainMenu => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home => self.move_selection_home(),
                KeyCode::End => self.move_selection_end(),
                KeyCode::Enter => {
                    self.handle_main_menu_selection().await;
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    if self.providers.len() > 1 {
                        self.state = AppState::ProviderSelection;
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                        self.update_provider_items();
                    } else {
                        return Some(Action::Quit);
                    }
                }
                _ => {}
            },
            AppState::CategorySelection(content_type) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home => self.move_selection_home(),
                KeyCode::End => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index < self.categories.len() {
                        let category = self.categories[self.selected_index].clone();
                        self.load_streams(content_type.clone(), category).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    self.state = AppState::MainMenu;
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.update_main_menu_items();
                }
                _ => {}
            },
            AppState::StreamSelection(content_type, _category) => {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                    KeyCode::PageUp => self.move_selection_page_up(),
                    KeyCode::PageDown => self.move_selection_page_down(),
                    KeyCode::Home => self.move_selection_home(),
                    KeyCode::End => self.move_selection_end(),
                    // TODO: Add favourite toggling when available
                    KeyCode::Enter => {
                        if self.selected_index < self.streams.len() {
                            let stream = self.streams[self.selected_index].clone();
                            match content_type {
                                ContentType::Series => {
                                    self.load_seasons(stream).await;
                                }
                                _ => {
                                    self.play_stream(&stream).await;
                                }
                            }
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('b') => {
                        self.state = AppState::CategorySelection(content_type.clone());
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                    }
                    _ => {}
                }
            }
            AppState::SeasonSelection(series) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home => self.move_selection_home(),
                KeyCode::End => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index < self.seasons.len() {
                        let season = &self.seasons[self.selected_index];
                        self.load_episodes(series.clone(), season.clone()).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    if let Some(category) = self.categories.iter().find(|c| {
                        self.streams
                            .iter()
                            .any(|s| s.category_id == Some(c.category_id.clone()))
                    }) {
                        self.state =
                            AppState::StreamSelection(ContentType::Series, category.clone());
                        self.selected_index = 0;
                        self.scroll_offset = 0;
                    }
                }
                _ => {}
            },
            AppState::EpisodeSelection(series, _season) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home => self.move_selection_home(),
                KeyCode::End => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index < self.episodes.len() {
                        let episode = self.episodes[self.selected_index].clone();
                        self.play_episode(&episode).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    self.state = AppState::SeasonSelection(series.clone());
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                }
                _ => {}
            },
            AppState::FavouriteSelection => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home => self.move_selection_home(),
                KeyCode::End => self.move_selection_end(),
                KeyCode::Char('f') => {
                    if self.selected_index < self.favourites.len() {
                        self.remove_favourite(self.selected_index).await;
                    }
                }
                KeyCode::Enter => {
                    if self.selected_index < self.favourites.len() {
                        let fav = self.favourites[self.selected_index].clone();
                        self.play_favourite(&fav).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    self.state = AppState::MainMenu;
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.update_main_menu_items();
                }
                _ => {}
            },
            AppState::Playing(_name) => match key.code {
                KeyCode::Esc | KeyCode::Char('s') => {
                    self.stop_playing();
                }
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    fn move_selection_down(&mut self) {
        let max_index = self.items.len().saturating_sub(1);
        if self.selected_index < max_index {
            self.selected_index += 1;
            let visible_height = 20; // Adjust based on terminal height
            if self.selected_index >= self.scroll_offset + visible_height {
                self.scroll_offset = self.selected_index - visible_height + 1;
            }
        }
    }

    fn move_selection_page_up(&mut self) {
        let page_size = 10; // Move 10 items at a time
        if self.selected_index > 0 {
            self.selected_index = self.selected_index.saturating_sub(page_size);
            if self.selected_index < self.scroll_offset {
                self.scroll_offset = self.selected_index;
            }
        }
    }

    fn move_selection_page_down(&mut self) {
        let page_size = 10; // Move 10 items at a time
        let max_index = self.items.len().saturating_sub(1);
        if self.selected_index < max_index {
            self.selected_index = (self.selected_index + page_size).min(max_index);
            let visible_height = 20;
            if self.selected_index >= self.scroll_offset + visible_height {
                self.scroll_offset = self.selected_index - visible_height + 1;
            }
        }
    }

    fn move_selection_home(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn move_selection_end(&mut self) {
        if !self.items.is_empty() {
            self.selected_index = self.items.len() - 1;
            let visible_height = 20;
            if self.items.len() > visible_height {
                self.scroll_offset = self.items.len() - visible_height;
            } else {
                self.scroll_offset = 0;
            }
        }
    }

    async fn connect_to_provider(&mut self, provider: &ProviderConfig) {
        self.state = AppState::Loading(format!(
            "Connecting to {}...",
            provider.name.as_ref().unwrap_or(&provider.url)
        ));

        self.add_log(format!(
            "Connecting to provider: {}",
            provider.name.as_ref().unwrap_or(&provider.url)
        ));

        match XTreamAPI::new(
            provider.url.clone(),
            provider.username.clone(),
            provider.password.clone(),
            provider.name.clone(),
        ) {
            Ok(api) => {
                self.current_api = Some(api);
                self.state = AppState::MainMenu;
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.update_main_menu_items();
                self.add_log("Successfully connected to provider".to_string());
            }
            Err(e) => {
                self.state = AppState::Error(format!("Failed to connect: {}", e));
                self.add_log(format!("Connection failed: {}", e));
            }
        }
    }

    fn update_provider_items(&mut self) {
        self.items = self
            .providers
            .iter()
            .map(|p| p.name.clone().unwrap_or_else(|| p.url.clone()))
            .collect();
    }

    fn update_main_menu_items(&mut self) {
        self.items = vec![
            "🌟 Favourites".to_string(),
            "Live TV".to_string(),
            "Movies (VOD)".to_string(),
            "TV Series".to_string(),
            "Refresh Cache".to_string(),
            "Clear Cache".to_string(),
        ];
    }

    async fn handle_main_menu_selection(&mut self) {
        match self.selected_index {
            0 => self.load_favourites().await,
            1 => self.load_categories(ContentType::Live).await,
            2 => self.load_categories(ContentType::Movies).await,
            3 => self.load_categories(ContentType::Series).await,
            4 => self.refresh_cache().await,
            5 => self.clear_cache().await,
            _ => {}
        }
    }

    async fn load_categories(&mut self, content_type: ContentType) {
        self.state = AppState::Loading(format!("Loading {} categories...", content_type));
        self.add_log(format!("Loading {} categories", content_type));

        if let Some(api) = &mut self.current_api {
            let result = match content_type {
                ContentType::Live => api.get_live_categories().await,
                ContentType::Movies => api.get_vod_categories().await,
                ContentType::Series => api.get_series_categories().await,
            };

            match result {
                Ok(categories) => {
                    self.categories = categories;
                    self.items = self
                        .categories
                        .iter()
                        .map(|c| c.category_name.clone())
                        .collect();
                    self.state = AppState::CategorySelection(content_type);
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.add_log(format!("Loaded {} categories", self.categories.len()));
                }
                Err(e) => {
                    self.state = AppState::Error(format!("Failed to load categories: {}", e));
                    self.add_log(format!("Failed to load categories: {}", e));
                }
            }
        }
    }

    async fn load_streams(&mut self, content_type: ContentType, category: Category) {
        self.state = AppState::Loading(format!(
            "Loading streams from {}...",
            category.category_name
        ));
        self.add_log(format!(
            "Loading streams from category: {}",
            category.category_name
        ));

        if let Some(api) = &mut self.current_api {
            let result = match content_type {
                ContentType::Live => api.get_live_streams(Some(&category.category_id)).await,
                ContentType::Movies => api.get_vod_streams(Some(&category.category_id)).await,
                ContentType::Series => {
                    api.get_series(Some(&category.category_id))
                        .await
                        .map(|series_infos| {
                            series_infos
                                .into_iter()
                                .map(|info| Stream {
                                    num: info.num,
                                    name: info.name.clone(),
                                    stream_type: "series".to_string(),
                                    stream_id: info.series_id,
                                    stream_icon: info.cover.clone(),
                                    epg_channel_id: None,
                                    added: None,
                                    category_id: info.category_id.clone(),
                                    category_ids: None,
                                    custom_sid: None,
                                    tv_archive: None,
                                    direct_source: None,
                                    tv_archive_duration: None,
                                    is_adult: None,
                                    container_extension: None,
                                    rating: None,
                                    rating_5based: None,
                                })
                                .collect()
                        })
                }
            };

            match result {
                Ok(streams) => {
                    self.streams = streams;
                    self.items = self.streams.iter().map(|s| s.name.clone()).collect();
                    self.state = AppState::StreamSelection(content_type, category);
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.add_log(format!("Loaded {} streams", self.streams.len()));
                }
                Err(e) => {
                    self.state = AppState::Error(format!("Failed to load streams: {}", e));
                    self.add_log(format!("Failed to load streams: {}", e));
                }
            }
        }
    }

    async fn load_seasons(&mut self, series: Stream) {
        self.state = AppState::Loading(format!("Loading seasons for {}...", series.name));
        self.add_log(format!("Loading seasons for: {}", series.name));

        if let Some(api) = &mut self.current_api {
            match api.get_series_info(series.stream_id).await {
                Ok(info) => {
                    if let Some(episodes) = &info.episodes {
                        self.seasons = episodes
                            .keys()
                            .map(|season_num| TuiSeason {
                                season_number: season_num.parse().unwrap_or(0),
                                name: format!("Season {}", season_num),
                                episode_count: episodes
                                    .get(season_num)
                                    .map(|eps| eps.len())
                                    .unwrap_or(0),
                            })
                            .collect();
                    } else {
                        self.seasons = Vec::new();
                    }

                    self.items = self
                        .seasons
                        .iter()
                        .map(|s| format!("{} ({} episodes)", s.name, s.episode_count))
                        .collect();

                    self.state = AppState::SeasonSelection(series);
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.add_log(format!("Loaded {} seasons", self.seasons.len()));
                }
                Err(e) => {
                    self.state = AppState::Error(format!("Failed to load seasons: {}", e));
                    self.add_log(format!("Failed to load seasons: {}", e));
                }
            }
        }
    }

    async fn load_episodes(&mut self, series: Stream, season: TuiSeason) {
        self.state = AppState::Loading(format!("Loading episodes for {}...", season.name));
        self.add_log(format!(
            "Loading episodes for {} - {}",
            series.name, season.name
        ));

        if let Some(api) = &mut self.current_api {
            match api.get_series_info(series.stream_id).await {
                Ok(info) => {
                    if let Some(episodes_map) = &info.episodes {
                        if let Some(episodes) = episodes_map.get(&season.season_number.to_string())
                        {
                            self.episodes = episodes.clone();
                            self.items = self
                                .episodes
                                .iter()
                                .map(|e| format!("Episode {}: {}", e.episode_num, e.title))
                                .collect();

                            self.state = AppState::EpisodeSelection(series, season);
                            self.selected_index = 0;
                            self.scroll_offset = 0;
                            self.add_log(format!("Loaded {} episodes", self.episodes.len()));
                        } else {
                            self.state =
                                AppState::Error("No episodes found for this season".to_string());
                        }
                    } else {
                        self.state = AppState::Error("No episodes available".to_string());
                    }
                }
                Err(e) => {
                    self.state = AppState::Error(format!("Failed to load episodes: {}", e));
                    self.add_log(format!("Failed to load episodes: {}", e));
                }
            }
        }
    }

    async fn load_favourites(&mut self) {
        self.state = AppState::Loading("Loading favourites...".to_string());
        self.add_log("Loading favourites".to_string());

        if let Some(api) = &mut self.current_api {
            match api.cache_manager.get_favourites(&api.provider_hash).await {
                Ok(favs) => {
                    self.favourites = favs;
                    self.items = self
                        .favourites
                        .iter()
                        .map(|f| format!("[{}] {}", f.stream_type, f.name))
                        .collect();

                    self.state = AppState::FavouriteSelection;
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.add_log(format!("Loaded {} favourites", self.favourites.len()));
                }
                Err(e) => {
                    self.state = AppState::Error(format!("Failed to load favourites: {}", e));
                    self.add_log(format!("Failed to load favourites: {}", e));
                }
            }
        }
    }

    // TODO: Implement favourite toggling when Stream has is_favourite field

    async fn remove_favourite(&mut self, index: usize) {
        if index < self.favourites.len() {
            if let Some(api) = &self.current_api {
                let fav = &self.favourites[index];
                let _ = api
                    .cache_manager
                    .remove_favourite(&api.provider_hash, fav.stream_id, &fav.stream_type)
                    .await;
                self.add_log(format!("Removed {} from favourites", fav.name));

                self.favourites.remove(index);
                self.items.remove(index);

                if self.selected_index >= self.items.len() && self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
        }
    }

    async fn play_stream(&mut self, stream: &Stream) {
        self.state = AppState::Playing(stream.name.clone());
        self.add_log(format!("Playing: {}", stream.name));

        if let Some(api) = &self.current_api {
            let url = api.get_stream_url(
                stream.stream_id,
                if stream.stream_type == "live" {
                    "live"
                } else {
                    "movie"
                },
                stream.container_extension.as_deref(),
            );

            // Use TUI-specific play method that runs in background
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play stream: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Player started in background window".to_string());
                self.add_log("Press 's' or Esc to stop playback".to_string());
            }
        }
    }

    async fn play_episode(&mut self, episode: &ApiEpisode) {
        self.state = AppState::Playing(episode.title.clone());
        self.add_log(format!("Playing: {}", episode.title));

        if let Some(api) = &self.current_api {
            let url = api.get_stream_url(
                episode.id.parse().unwrap_or(0),
                "series",
                episode.container_extension.as_deref(),
            );

            // Use TUI-specific play method that runs in background
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play episode: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Player started in background window".to_string());
                self.add_log("Press 's' or Esc to stop playback".to_string());
            }
        }
    }

    async fn play_favourite(&mut self, fav: &FavouriteStream) {
        self.state = AppState::Playing(fav.name.clone());
        self.add_log(format!("Playing favourite: {}", fav.name));

        if let Some(api) = &self.current_api {
            // Construct the stream URL from the favourite stream ID
            let url = api.get_stream_url(fav.stream_id, &fav.stream_type, None);

            // Use TUI-specific play method that runs in background
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play favourite: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Player started in background window".to_string());
                self.add_log("Press 's' or Esc to stop playback".to_string());
            }
        }
    }

    fn stop_playing(&mut self) {
        // Stop the player process
        let player = self.player.clone();
        tokio::spawn(async move {
            let _ = player.stop_tui().await;
        });

        self.state = AppState::MainMenu;
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.update_main_menu_items();
        self.add_log("Stopped playback".to_string());
    }

    async fn refresh_cache(&mut self) {
        let provider_hash = if let Some(api) = &self.current_api {
            api.provider_hash.clone()
        } else {
            return;
        };

        self.state = AppState::Loading("Refreshing cache...".to_string());
        self.add_log("Refreshing cache".to_string());

        if let Some(api) = &self.current_api {
            if let Err(e) = api.cache_manager.clear_provider_cache(&provider_hash).await {
                self.state = AppState::Error(format!("Failed to refresh cache: {}", e));
                self.add_log(format!("Cache refresh failed: {}", e));
            } else {
                self.state = AppState::MainMenu;
                self.update_main_menu_items();
                self.add_log("Cache refreshed successfully".to_string());
            }
        }
    }

    async fn clear_cache(&mut self) {
        self.state = AppState::Loading("Clearing cache...".to_string());
        self.add_log("Clearing all cache".to_string());

        if let Some(api) = &self.current_api {
            if let Err(e) = api.cache_manager.clear_all_cache().await {
                self.state = AppState::Error(format!("Failed to clear cache: {}", e));
                self.add_log(format!("Cache clear failed: {}", e));
            } else {
                self.state = AppState::MainMenu;
                self.update_main_menu_items();
                self.add_log("Cache cleared successfully".to_string());
            }
        }
    }

    fn add_log(&mut self, message: String) {
        self.logs.push((Instant::now(), message));
        // Keep only last 100 logs
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }
}
