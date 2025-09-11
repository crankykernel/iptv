// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use crate::cli::menu::ContentType;
use crate::config::ProviderConfig;
use crate::player::Player;
use crate::xtream_api::{
    ApiEpisode, Category, FavouriteStream, Stream, VodInfoResponse, XTreamAPI,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum LogDisplayMode {
    Side,
    None,
    Full,
}

#[derive(Debug, Clone)]
pub struct TuiSeason {
    pub season_number: u32,
    pub name: String,
    pub episode_count: usize,
}

#[derive(Debug, Clone)]
pub struct VodInfoState {
    pub stream: Stream,
    pub saved_filter: String,
    pub saved_selected: usize,
    pub saved_filtered_indices: Vec<usize>,
    pub saved_scroll: usize,
    pub saved_items: Vec<String>,
    pub content_scroll: usize, // scroll position for content display
}

#[derive(Debug, Clone)]
pub enum AppState {
    ProviderSelection,
    MainMenu,
    CategorySelection(ContentType),
    StreamSelection(ContentType, Category),
    VodInfo(VodInfoState),
    SeasonSelection(Stream),
    EpisodeSelection(Stream, TuiSeason),
    FavouriteSelection,
    CrossProviderFavourites,
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
    pub log_display_mode: LogDisplayMode,
    pub log_selected_index: usize,
    pub log_scroll_offset: usize,
    pub page_size: usize,
    pub search_query: String,
    pub search_active: bool,
    pub filtered_indices: Vec<usize>,
    categories: Vec<Category>,
    streams: Vec<Stream>,
    seasons: Vec<TuiSeason>,
    episodes: Vec<ApiEpisode>,
    favourites: Vec<FavouriteStream>,
    cross_provider_favourites: Vec<(FavouriteStream, ProviderConfig)>,
    vod_info: Option<VodInfoResponse>,
    // Cache for categories by content type
    cached_categories: HashMap<ContentType, Vec<Category>>,
    // Cache for streams by content type and category ID
    cached_streams: HashMap<(ContentType, String), Vec<Stream>>,
}

impl App {
    pub fn new(providers: Vec<ProviderConfig>, player: Player) -> Self {
        let items = if providers.len() > 1 {
            let mut items = vec!["All Favourites".to_string()];
            items.extend(
                providers
                    .iter()
                    .map(|p| p.name.clone().unwrap_or_else(|| p.url.clone())),
            );
            items
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

        let filtered_indices = (0..items.len()).collect();

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
            log_display_mode: LogDisplayMode::Side,
            log_selected_index: 0,
            log_scroll_offset: 0,
            page_size: 20,
            search_query: String::new(),
            search_active: false,
            filtered_indices,
            categories: Vec::new(),
            streams: Vec::new(),
            seasons: Vec::new(),
            episodes: Vec::new(),
            favourites: Vec::new(),
            cross_provider_favourites: Vec::new(),
            vod_info: None,
            cached_categories: HashMap::new(),
            cached_streams: HashMap::new(),
        }
    }

    pub fn tick(&mut self) {
        // Update any time-based UI elements here
        // Note: Player status check moved to async tick method in run_app
    }

    pub async fn async_tick(&mut self) {
        // Check player status periodically to detect exits
        if matches!(self.state, AppState::Playing(_)) {
            let (is_running, exit_message) = self.player.check_player_status().await;

            if !is_running {
                // Return to main menu when player exits
                self.state = AppState::MainMenu;
                self.selected_index = 0;
                self.scroll_offset = 0;
                self.update_main_menu_items();

                if let Some(message) = exit_message {
                    self.add_log(format!("⚠️ {}", message));
                }
            }
        }
    }

    pub async fn handle_key_event(&mut self, key: KeyEvent) -> Option<Action> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Some(Action::Quit);
        }

        // Toggle log panel with Ctrl+.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('.') {
            self.log_display_mode = match self.log_display_mode {
                LogDisplayMode::Side => LogDisplayMode::None,
                LogDisplayMode::None => LogDisplayMode::Full,
                LogDisplayMode::Full => LogDisplayMode::Side,
            };
            self.add_log(match self.log_display_mode {
                LogDisplayMode::Side => "Log panel: side view".to_string(),
                LogDisplayMode::None => "Log panel: hidden".to_string(),
                LogDisplayMode::Full => "Log panel: full window".to_string(),
            });
            return None;
        }

        // Handle log scrolling when in full window mode
        if matches!(self.log_display_mode, LogDisplayMode::Full) {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.log_selected_index > 0 {
                        self.log_selected_index -= 1;
                        // Adjust scroll to keep selected line visible
                        if self.log_selected_index < self.log_scroll_offset {
                            self.log_scroll_offset = self.log_selected_index;
                        }
                    }
                    return None;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.log_selected_index < self.logs.len().saturating_sub(1) {
                        self.log_selected_index += 1;
                        // Adjust scroll to keep selected line visible (will be calculated in UI)
                    }
                    return None;
                }
                KeyCode::PageUp => {
                    let page_size = 10;
                    self.log_selected_index = self.log_selected_index.saturating_sub(page_size);
                    if self.log_selected_index < self.log_scroll_offset {
                        self.log_scroll_offset = self.log_selected_index;
                    }
                    return None;
                }
                KeyCode::PageDown => {
                    let page_size = 10;
                    let max_index = self.logs.len().saturating_sub(1);
                    self.log_selected_index = (self.log_selected_index + page_size).min(max_index);
                    return None;
                }
                KeyCode::Home | KeyCode::Char('H') => {
                    self.log_selected_index = 0;
                    self.log_scroll_offset = 0;
                    return None;
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.log_selected_index = self.logs.len().saturating_sub(1);
                    return None;
                }
                KeyCode::Esc => {
                    // Exit full log mode back to side panel
                    self.log_display_mode = LogDisplayMode::Side;
                    self.add_log("Log panel: side view".to_string());
                    return None;
                }
                _ => {
                    // Consume all other keys in full log mode to prevent them from 
                    // triggering actions in the underlying screens
                    return None;
                }
            }
        }

        // Handle search mode input
        if self.search_active {
            match key.code {
                KeyCode::Esc => {
                    self.cancel_search();
                    return None;
                }
                KeyCode::Enter => {
                    self.confirm_search();
                    return None;
                }
                KeyCode::Backspace => {
                    self.delete_search_char();
                    return None;
                }
                KeyCode::Char(c) => {
                    self.update_search(c);
                    return None;
                }
                _ => return None,
            }
        }

        // Start search on '/' key
        if key.code == KeyCode::Char('/')
            && !matches!(self.state, AppState::Loading(_) | AppState::Playing(_))
        {
            self.start_search();
            return None;
        }

        // Global stop playback key
        if key.code == KeyCode::Char('s') {
            self.stop_playing();
            self.add_log("Stopping any active playback".to_string());
            return None;
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
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index == 0 {
                        // All Favourites selected
                        self.load_all_favourites().await;
                    } else if self.selected_index - 1 < self.providers.len() {
                        let provider = self.providers[self.selected_index - 1].clone();
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
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
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
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
                KeyCode::Char('r') => {
                    // Force refresh categories
                    let ct = content_type.clone();
                    self.add_log("Refreshing categories...".to_string());
                    self.load_categories_internal(ct, true).await;
                }
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
            AppState::StreamSelection(content_type, category) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
                KeyCode::Char('r') => {
                    // Force refresh streams
                    let ct = content_type.clone();
                    let cat = category.clone();
                    self.add_log("Refreshing streams...".to_string());
                    self.load_streams_internal(ct, cat, true).await;
                }
                KeyCode::Char('f') => {
                    // selected_index already points to the correct stream
                    if self.selected_index < self.streams.len() {
                        let stream = self.streams[self.selected_index].clone();
                        self.toggle_favourite_stream(&stream).await;
                    }
                }
                KeyCode::Enter => {
                    // selected_index already points to the correct stream
                    if self.selected_index < self.streams.len() {
                        let stream = self.streams[self.selected_index].clone();
                        match content_type {
                            ContentType::Series => {
                                self.load_seasons(stream).await;
                            }
                            ContentType::Movies => {
                                // Save current filter and selected index before loading VOD info
                                let saved_filter = self.search_query.clone();
                                let saved_selected = self.selected_index;
                                let saved_filtered_indices = self.filtered_indices.clone();
                                let saved_scroll = self.scroll_offset;
                                let saved_items = self.items.clone();

                                // Load VOD info with saved state
                                self.load_vod_info_with_state(
                                    stream,
                                    saved_filter,
                                    saved_selected,
                                    saved_filtered_indices,
                                    saved_scroll,
                                    saved_items,
                                )
                                .await;
                            }
                            _ => {
                                self.play_stream(&stream).await;
                            }
                        }
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // Go back to category selection and reload categories
                    self.load_categories(content_type.clone()).await;
                }
                _ => {}
            },
            AppState::VodInfo(vod_state) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    // Always navigate through menu items
                    let menu_items: Vec<usize> = self
                        .items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            item.contains("Play Movie")
                                || item.contains("Copy URL")
                                || item.contains("Back")
                        })
                        .map(|(i, _)| i)
                        .collect();

                    if let Some(current_pos) =
                        menu_items.iter().position(|&i| i == self.selected_index)
                    {
                        if current_pos > 0 {
                            self.selected_index = menu_items[current_pos - 1];
                            self.ensure_selected_visible();
                        }
                    } else if let Some(&first_menu_item) = menu_items.first() {
                        // If no menu item is currently selected, select the first one
                        self.selected_index = first_menu_item;
                        self.ensure_selected_visible();
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    // Always navigate through menu items
                    let menu_items: Vec<usize> = self
                        .items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            item.contains("Play Movie")
                                || item.contains("Copy URL")
                                || item.contains("Back")
                        })
                        .map(|(i, _)| i)
                        .collect();

                    if let Some(current_pos) =
                        menu_items.iter().position(|&i| i == self.selected_index)
                    {
                        if current_pos < menu_items.len() - 1 {
                            self.selected_index = menu_items[current_pos + 1];
                            self.ensure_selected_visible();
                        }
                    } else if let Some(&first_menu_item) = menu_items.first() {
                        // If no menu item is currently selected, select the first one
                        self.selected_index = first_menu_item;
                        self.ensure_selected_visible();
                    }
                }
                KeyCode::PageUp => {
                    // Always scroll content up by page
                    if let AppState::VodInfo(state) = &mut self.state {
                        let visible_height = self.page_size;
                        state.content_scroll = state.content_scroll.saturating_sub(visible_height);
                    }
                }
                KeyCode::PageDown => {
                    // Always scroll content down by page
                    if let AppState::VodInfo(state) = &mut self.state {
                        let visible_height = self.page_size;
                        let max_scroll = self
                            .items
                            .len()
                            .saturating_sub(visible_height.min(self.items.len()));
                        state.content_scroll =
                            (state.content_scroll + visible_height).min(max_scroll);
                    }
                }
                KeyCode::Char(' ') => {
                    // Space - scroll down by page (or up if Shift is held)
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        // Shift+Space - scroll up by page
                        if let AppState::VodInfo(state) = &mut self.state {
                            let visible_height = self.page_size;
                            state.content_scroll =
                                state.content_scroll.saturating_sub(visible_height);
                        }
                    } else {
                        // Space - scroll down by page
                        if let AppState::VodInfo(state) = &mut self.state {
                            let visible_height = self.page_size;
                            let max_scroll = self
                                .items
                                .len()
                                .saturating_sub(visible_height.min(self.items.len()));
                            state.content_scroll =
                                (state.content_scroll + visible_height).min(max_scroll);
                        }
                    }
                }
                KeyCode::Home | KeyCode::Char('H') => {
                    // Always scroll content to top
                    if let AppState::VodInfo(state) = &mut self.state {
                        state.content_scroll = 0;
                    }
                }
                KeyCode::End | KeyCode::Char('G') => {
                    // Always scroll content to bottom
                    if let AppState::VodInfo(state) = &mut self.state {
                        let visible_height = self.page_size;
                        state.content_scroll = self
                            .items
                            .len()
                            .saturating_sub(visible_height.min(self.items.len()));
                    }
                }
                KeyCode::Enter => {
                    // Always execute selected menu action
                    let selected_item = &self.items[self.selected_index];

                    if selected_item.contains("Play Movie") {
                        self.play_vod_stream(&vod_state.stream.clone()).await;
                    } else if selected_item.contains("Copy URL") {
                        if let Some(api) = &self.current_api {
                            let extension = self
                                .vod_info
                                .as_ref()
                                .map(|info| info.movie_data.container_extension.as_str());
                            let url =
                                api.get_stream_url(vod_state.stream.stream_id, "movie", extension);
                            self.add_log(format!("Stream URL copied: {}", url));
                            self.status_message = Some("URL copied to logs!".to_string());
                        }
                    } else if selected_item.contains("Back") {
                        // Clone vod_state fields first to avoid borrow issues
                        let saved_filter = vod_state.saved_filter.clone();
                        let saved_selected = vod_state.saved_selected;
                        let saved_filtered_indices = vod_state.saved_filtered_indices.clone();
                        let saved_scroll = vod_state.saved_scroll;
                        let saved_items = vod_state.saved_items.clone();

                        // Go back to stream selection and restore previous state
                        let category =
                            if let Some(cat) = self.categories.iter().find(|c| {
                                vod_state.stream.category_id.as_ref() == Some(&c.category_id)
                            }) {
                                cat.clone()
                            } else {
                                // Fallback to all movies
                                Category {
                                    category_id: "all".to_string(),
                                    category_name: "All".to_string(),
                                    parent_id: None,
                                }
                            };

                        // Restore the previous state
                        self.state = AppState::StreamSelection(ContentType::Movies, category);
                        self.items = saved_items;
                        self.search_query = saved_filter;
                        self.selected_index = saved_selected;
                        self.filtered_indices = saved_filtered_indices;
                        self.scroll_offset = saved_scroll;

                        // If there was a filter active, update the status message
                        if !self.search_query.is_empty() {
                            self.status_message = Some(format!(
                                "Filtered: \"{}\" (Press '/' to search again)",
                                self.search_query
                            ));
                        }
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // Clone vod_state fields first to avoid borrow issues
                    let saved_filter = vod_state.saved_filter.clone();
                    let saved_selected = vod_state.saved_selected;
                    let saved_filtered_indices = vod_state.saved_filtered_indices.clone();
                    let saved_scroll = vod_state.saved_scroll;
                    let saved_items = vod_state.saved_items.clone();

                    // Quick back option - restore previous state
                    let category = if let Some(cat) = self
                        .categories
                        .iter()
                        .find(|c| vod_state.stream.category_id.as_ref() == Some(&c.category_id))
                    {
                        cat.clone()
                    } else {
                        Category {
                            category_id: "all".to_string(),
                            category_name: "All".to_string(),
                            parent_id: None,
                        }
                    };

                    // Restore the previous state
                    self.state = AppState::StreamSelection(ContentType::Movies, category);
                    self.items = saved_items;
                    self.search_query = saved_filter;
                    self.selected_index = saved_selected;
                    self.filtered_indices = saved_filtered_indices;
                    self.scroll_offset = saved_scroll;

                    // If there was a filter active, update the status message
                    if !self.search_query.is_empty() {
                        self.status_message = Some(format!(
                            "Filtered: \"{}\" (Press '/' to search again)",
                            self.search_query
                        ));
                    }
                }
                _ => {}
            },
            AppState::SeasonSelection(series) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index < self.seasons.len() {
                        let season = &self.seasons[self.selected_index];
                        self.load_episodes(series.clone(), season.clone()).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // Go back to stream selection and reload series
                    let category = self
                        .categories
                        .iter()
                        .find(|c| {
                            self.streams
                                .iter()
                                .any(|s| s.category_id == Some(c.category_id.clone()))
                        })
                        .cloned()
                        .unwrap_or_else(|| Category {
                            category_id: "all".to_string(),
                            category_name: "All".to_string(),
                            parent_id: None,
                        });

                    self.load_streams(ContentType::Series, category).await;
                }
                _ => {}
            },
            AppState::EpisodeSelection(series, _season) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
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
            AppState::CrossProviderFavourites => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::PageUp => self.move_selection_page_up(),
                KeyCode::PageDown => self.move_selection_page_down(),
                KeyCode::Home | KeyCode::Char('H') => self.move_selection_home(),
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
                KeyCode::Enter => {
                    if self.selected_index < self.cross_provider_favourites.len() {
                        let (favourite, provider) =
                            self.cross_provider_favourites[self.selected_index].clone();

                        // Connect to provider silently if needed (without changing state)
                        if self.current_api.is_none()
                            || self.current_api.as_ref().unwrap().provider_hash
                                != crate::XTreamAPI::new(
                                    provider.url.clone(),
                                    provider.username.clone(),
                                    provider.password.clone(),
                                    provider.name.clone(),
                                )
                                .unwrap()
                                .provider_hash
                        {
                            self.add_log(format!(
                                "Connecting to provider: {}",
                                provider.name.as_ref().unwrap_or(&provider.url)
                            ));

                            match crate::XTreamAPI::new(
                                provider.url.clone(),
                                provider.username.clone(),
                                provider.password.clone(),
                                provider.name.clone(),
                            ) {
                                Ok(mut api) => {
                                    api.disable_progress();
                                    self.current_api = Some(api);
                                    self.add_log("Successfully connected to provider".to_string());
                                }
                                Err(e) => {
                                    self.state =
                                        AppState::Error(format!("Failed to connect: {}", e));
                                    self.add_log(format!("Connection failed: {}", e));
                                    return None;
                                }
                            }
                        }

                        // Play the favourite using TUI-specific method
                        if let Some(api) = &self.current_api {
                            let stream_url = api.get_stream_url(
                                favourite.stream_id,
                                &favourite.stream_type,
                                None,
                            );

                            self.add_log(format!("Playing: {}", favourite.name));

                            // Use TUI-specific play method that runs in background
                            if let Err(e) = self.player.play_tui(&stream_url).await {
                                self.state =
                                    AppState::Error(format!("Failed to play favourite: {}", e));
                                self.add_log(format!("Playback failed: {}", e));
                            } else {
                                self.add_log("Player started in background window".to_string());
                                self.add_log("Continue browsing while video plays".to_string());
                                // Stay in CrossProviderFavourites state
                            }
                        }
                    }
                }
                KeyCode::Char('f') => {
                    if self.selected_index < self.cross_provider_favourites.len() {
                        let (favourite, provider) =
                            &self.cross_provider_favourites[self.selected_index];
                        let favourites_manager = match crate::FavouritesManager::new() {
                            Ok(fm) => fm,
                            Err(e) => {
                                self.add_log(format!("Failed to access favourites: {}", e));
                                return None;
                            }
                        };

                        let api = match crate::XTreamAPI::new(
                            provider.url.clone(),
                            provider.username.clone(),
                            provider.password.clone(),
                            provider.name.clone(),
                        ) {
                            Ok(mut api) => {
                                api.disable_progress();
                                api
                            }
                            Err(e) => {
                                self.add_log(format!("Failed to connect: {}", e));
                                return None;
                            }
                        };

                        let _ = favourites_manager.remove_favourite(
                            &api.provider_hash,
                            favourite.stream_id,
                            &favourite.stream_type,
                        );

                        self.add_log(format!("Removed {} from favourites", favourite.name));

                        // Reload the cross-provider favourites
                        self.load_all_favourites().await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    self.state = AppState::ProviderSelection;
                    self.selected_index = 0;
                    self.scroll_offset = 0;
                    self.update_provider_items();
                }
                KeyCode::Char('/') => {
                    self.search_active = true;
                    self.search_query.clear();
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
        let indices = if self.filtered_indices.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.filtered_indices.clone()
        };

        if indices.is_empty() {
            return;
        }

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            if current_pos > 0 {
                // Normal upward movement
                self.selected_index = indices[current_pos - 1];
                // Update scroll to follow selection
                let visible_pos = indices[0..current_pos]
                    .iter()
                    .filter(|&&idx| idx >= self.scroll_offset)
                    .count();
                if visible_pos == 0 {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                }
            } else {
                // Wrap to bottom: move to last item
                self.selected_index = indices[indices.len() - 1];
                // Update scroll to show the last item
                let visible_height = 20;
                if indices.len() > visible_height {
                    self.scroll_offset = indices.len() - visible_height;
                } else {
                    self.scroll_offset = 0;
                }
            }
        }
    }

    fn move_selection_down(&mut self) {
        let indices = if self.filtered_indices.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.filtered_indices.clone()
        };

        if indices.is_empty() {
            return;
        }

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            if current_pos < indices.len() - 1 {
                // Normal downward movement
                self.selected_index = indices[current_pos + 1];
                // Update scroll to follow selection
                let visible_height = 20;
                if current_pos + 1 >= self.scroll_offset + visible_height {
                    self.scroll_offset = current_pos + 1 - visible_height + 1;
                }
            } else {
                // Wrap to top: move to first item
                self.selected_index = indices[0];
                // Reset scroll to top
                self.scroll_offset = 0;
            }
        }
    }

    fn move_selection_page_up(&mut self) {
        let indices = if self.filtered_indices.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.filtered_indices.clone()
        };

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            let new_pos = current_pos.saturating_sub(10);
            self.selected_index = indices[new_pos];
            if new_pos < self.scroll_offset {
                self.scroll_offset = new_pos;
            }
        }
    }

    fn move_selection_page_down(&mut self) {
        let indices = if self.filtered_indices.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.filtered_indices.clone()
        };

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            let new_pos = (current_pos + 10).min(indices.len() - 1);
            self.selected_index = indices[new_pos];
            let visible_height = 20;
            if new_pos >= self.scroll_offset + visible_height {
                self.scroll_offset = new_pos - visible_height + 1;
            }
        }
    }

    fn move_selection_home(&mut self) {
        let indices = if self.filtered_indices.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.filtered_indices.clone()
        };

        if !indices.is_empty() {
            self.selected_index = indices[0];
            self.scroll_offset = 0;
        }
    }

    fn move_selection_end(&mut self) {
        let indices = if self.filtered_indices.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.filtered_indices.clone()
        };

        if !indices.is_empty() {
            self.selected_index = indices[indices.len() - 1];
            let visible_height = 20;
            if indices.len() > visible_height {
                self.scroll_offset = indices.len() - visible_height;
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
            Ok(mut api) => {
                // Set up logger for TUI mode
                api.disable_progress();
                // Note: We can't actually pass a closure that captures self here due to lifetime issues
                // Instead we'll just disable progress bars for now
                self.current_api = Some(api);
                // Clear caches when switching providers
                self.cached_categories.clear();
                self.cached_streams.clear();
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
        let mut items = vec!["All Favourites".to_string()];
        items.extend(
            self.providers
                .iter()
                .map(|p| p.name.clone().unwrap_or_else(|| p.url.clone())),
        );
        self.items = items;
        self.reset_filter();
    }

    fn update_main_menu_items(&mut self) {
        self.items = vec![
            "Favourites".to_string(),
            "Live TV".to_string(),
            "Movies (VOD)".to_string(),
            "TV Series".to_string(),
            "Refresh Cache".to_string(),
            "Clear Cache".to_string(),
        ];
        self.reset_filter();
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
        self.load_categories_internal(content_type, false).await;
    }

    async fn load_categories_internal(&mut self, content_type: ContentType, force_refresh: bool) {
        // Check cache first if not forcing refresh
        if !force_refresh && let Some(cached) = self.cached_categories.get(&content_type) {
            let ct = content_type.clone();
            self.categories = cached.clone();
            self.add_log(format!("Using cached {} categories", ct));
            self.items = self
                .categories
                .iter()
                .map(|c| c.category_name.clone())
                .collect();
            self.reset_filter();
            self.state = AppState::CategorySelection(content_type.clone());
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }

        self.state = AppState::Loading(format!("Loading {} categories...", content_type));
        self.add_log(format!("Loading {} categories", content_type));

        if let Some(api) = &mut self.current_api {
            let result = match content_type {
                ContentType::Live => api.get_live_categories().await,
                ContentType::Movies => api.get_vod_categories().await,
                ContentType::Series => api.get_series_categories().await,
            };

            match result {
                Ok(mut categories) => {
                    // Add "All" category at the beginning
                    let all_category = Category {
                        category_id: "all".to_string(),
                        category_name: "All".to_string(),
                        parent_id: None,
                    };
                    categories.insert(0, all_category);

                    // Store in cache
                    self.cached_categories
                        .insert(content_type.clone(), categories.clone());

                    self.categories = categories;
                    self.items = self
                        .categories
                        .iter()
                        .map(|c| c.category_name.clone())
                        .collect();
                    self.reset_filter();
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
        self.load_streams_internal(content_type, category, false)
            .await;
    }

    async fn load_streams_internal(
        &mut self,
        content_type: ContentType,
        category: Category,
        force_refresh: bool,
    ) {
        // Check cache first if not forcing refresh
        let cache_key = (content_type.clone(), category.category_id.clone());
        if !force_refresh && let Some(cached) = self.cached_streams.get(&cache_key) {
            let cat_name = category.category_name.clone();
            self.streams = cached.clone();
            self.add_log(format!("Using cached streams for {}", cat_name));

            // Get list of favourites to mark them with a star
            let favourites = if let Some(api) = &self.current_api {
                api.favourites_manager
                    .get_favourites(&api.provider_hash)
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            // Create item list with stars for favourites
            self.items = self
                .streams
                .iter()
                .map(|s| {
                    let is_favourite = favourites.iter().any(|f| f.stream_id == s.stream_id);
                    if is_favourite {
                        format!("⭐ {}", s.name)
                    } else {
                        s.name.clone()
                    }
                })
                .collect();

            self.reset_filter();
            self.state = AppState::StreamSelection(content_type, category);
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }

        self.state = AppState::Loading(format!(
            "Loading streams from {}...",
            category.category_name
        ));
        self.add_log(format!(
            "Loading streams from category: {}",
            category.category_name
        ));

        if let Some(api) = &mut self.current_api {
            // Pass None for "All" category to get all streams
            let category_id = if category.category_id == "all" {
                None
            } else {
                Some(category.category_id.as_str())
            };

            let result = match content_type {
                ContentType::Live => api.get_live_streams(category_id).await,
                ContentType::Movies => api.get_vod_streams(category_id).await,
                ContentType::Series => api.get_series(category_id).await.map(|series_infos| {
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
                }),
            };

            match result {
                Ok(streams) => {
                    // Store in cache
                    self.cached_streams.insert(cache_key, streams.clone());

                    self.streams = streams;

                    // Get list of favourites to mark them with a star
                    let favourites = if let Some(api) = &self.current_api {
                        api.favourites_manager
                            .get_favourites(&api.provider_hash)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    // Create item list with stars for favourites
                    self.items = self
                        .streams
                        .iter()
                        .map(|s| {
                            let is_favourite =
                                favourites.iter().any(|f| f.stream_id == s.stream_id);
                            if is_favourite {
                                format!("[FAV] {}", s.name)
                            } else {
                                s.name.clone()
                            }
                        })
                        .collect();

                    self.reset_filter();
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
                    self.reset_filter();

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
                            self.reset_filter();

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

    async fn load_all_favourites(&mut self) {
        self.state = AppState::Loading("Loading all favourites...".to_string());
        self.add_log("Loading favourites from all providers".to_string());

        let favourites_manager = match crate::FavouritesManager::new() {
            Ok(fm) => fm,
            Err(e) => {
                self.state = AppState::Error(format!("Failed to access favourites: {}", e));
                return;
            }
        };

        let mut all_favourites = Vec::new();
        let mut all_items = Vec::new();

        // Collect favourites from all providers
        let providers = self.providers.clone();
        for provider in &providers {
            let api = match crate::XTreamAPI::new(
                provider.url.clone(),
                provider.username.clone(),
                provider.password.clone(),
                provider.name.clone(),
            ) {
                Ok(mut api) => {
                    api.disable_progress();
                    api
                }
                Err(e) => {
                    self.add_log(format!("Failed to connect to provider: {}", e));
                    continue;
                }
            };

            match favourites_manager.get_favourites(&api.provider_hash) {
                Ok(favs) => {
                    for fav in favs {
                        let provider_name = provider.name.as_ref().unwrap_or(&provider.url);
                        all_items.push(format!(
                            "[{}] {} [{}]",
                            fav.stream_type, fav.name, provider_name
                        ));
                        all_favourites.push((fav, provider.clone()));
                    }
                }
                Err(e) => {
                    self.add_log(format!("Failed to load favourites: {}", e));
                }
            }
        }

        if all_favourites.is_empty() {
            self.state = AppState::Error("No favourites found across any provider".to_string());
            return;
        }

        // Store the cross-provider favourites
        self.cross_provider_favourites = all_favourites;
        self.items = all_items;
        self.reset_filter();

        self.state = AppState::CrossProviderFavourites;
        self.selected_index = 0;
        self.scroll_offset = 0;

        self.add_log(format!("Loaded {} favourites", self.items.len()));
    }

    async fn load_favourites(&mut self) {
        self.state = AppState::Loading("Loading favourites...".to_string());
        self.add_log("Loading favourites".to_string());

        if let Some(api) = &mut self.current_api {
            match api.favourites_manager.get_favourites(&api.provider_hash) {
                Ok(favs) => {
                    self.favourites = favs;
                    self.items = self
                        .favourites
                        .iter()
                        .map(|f| format!("[{}] {}", f.stream_type, f.name))
                        .collect();
                    self.reset_filter();

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

    async fn toggle_favourite_stream(&mut self, stream: &Stream) {
        if let Some(api) = &self.current_api {
            // Check if this stream is already a favourite
            let favourites = api
                .favourites_manager
                .get_favourites(&api.provider_hash)
                .unwrap_or_default();
            let is_favourite = favourites.iter().any(|f| f.stream_id == stream.stream_id);

            if is_favourite {
                // Remove from favourites
                let _ = api.favourites_manager.remove_favourite(
                    &api.provider_hash,
                    stream.stream_id,
                    &stream.stream_type,
                );
                self.add_log(format!("Removed {} from favourites", stream.name));

                // Update the display to show the star is removed
                if let Some(item) = self.items.get_mut(self.selected_index)
                    && item.starts_with("[FAV] ")
                {
                    *item = item[6..].to_string(); // Remove the fav prefix
                }
            } else {
                // Add to favourites
                let favourite = crate::xtream_api::FavouriteStream {
                    stream_id: stream.stream_id,
                    name: stream.name.clone(),
                    stream_type: stream.stream_type.clone(),
                    provider_hash: api.provider_hash.clone(),
                    added_date: chrono::Utc::now(),
                    category_id: stream.category_id.clone(),
                };

                let _ = api
                    .favourites_manager
                    .add_favourite(&api.provider_hash, favourite);
                self.add_log(format!("Added {} to favourites", stream.name));

                // Update the display to show the star
                if let Some(item) = self.items.get_mut(self.selected_index)
                    && !item.starts_with("[FAV] ")
                {
                    *item = format!("[FAV] {}", item);
                }
            }
        }
    }

    async fn remove_favourite(&mut self, index: usize) {
        if index < self.favourites.len()
            && let Some(api) = &self.current_api
        {
            let fav = &self.favourites[index];
            let _ = api.favourites_manager.remove_favourite(
                &api.provider_hash,
                fav.stream_id,
                &fav.stream_type,
            );
            self.add_log(format!("Removed {} from favourites", fav.name));

            self.favourites.remove(index);
            self.items.remove(index);

            // Update filtered_indices after removing item
            self.filtered_indices = (0..self.items.len()).collect();

            // Adjust selected index if needed
            if self.selected_index >= self.items.len() && self.selected_index > 0 {
                self.selected_index -= 1;
            }

            // Ensure scroll offset is valid
            if self.scroll_offset > 0 && self.scroll_offset >= self.items.len() {
                self.scroll_offset = self.items.len().saturating_sub(1);
            }
        }
    }

    async fn play_stream(&mut self, stream: &Stream) {
        // Store the current state to return to after starting playback
        let return_state = self.state.clone();

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

            // Log the stream URL to the logs panel
            self.add_log(format!("Stream URL: {}", url));

            // Use TUI-specific play method that runs in background
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play stream: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Player started in background window".to_string());
                self.add_log("Continue browsing while video plays".to_string());
                // Return to the previous state so user can continue browsing
                self.state = return_state;
            }
        }
    }

    async fn play_episode(&mut self, episode: &ApiEpisode) {
        // Store the current state to return to after starting playback
        let return_state = self.state.clone();

        self.add_log(format!("Playing: {}", episode.title));

        if let Some(api) = &self.current_api {
            let url = api.get_stream_url(
                episode.id.parse().unwrap_or(0),
                "series",
                episode.container_extension.as_deref(),
            );

            // Log the stream URL to the logs panel
            self.add_log(format!("Stream URL: {}", url));

            // Use TUI-specific play method that runs in background
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play episode: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Player started in background window".to_string());
                self.add_log("Continue browsing while video plays".to_string());
                // Return to the previous state so user can continue browsing
                self.state = return_state;
            }
        }
    }

    async fn play_favourite(&mut self, fav: &FavouriteStream) {
        // Store the current state to return to after starting playback
        let return_state = self.state.clone();

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
                self.add_log("Continue browsing while video plays".to_string());
                // Return to the previous state so user can continue browsing
                self.state = return_state;
            }
        }
    }

    async fn load_vod_info_with_state(
        &mut self,
        stream: Stream,
        saved_filter: String,
        saved_selected: usize,
        saved_filtered_indices: Vec<usize>,
        saved_scroll: usize,
        saved_items: Vec<String>,
    ) {
        self.state = AppState::Loading(format!("Loading info for: {}", stream.name));
        self.add_log(format!("Fetching VOD info for: {}", stream.name));
        self.add_log("Connecting to server...".to_string());
        self.add_log("Requesting movie details...".to_string());

        let vod_result = if let Some(api) = &mut self.current_api {
            api.get_vod_info(stream.stream_id).await
        } else {
            return;
        };

        match vod_result {
            Ok(vod_info) => {
                self.add_log("Successfully loaded movie information".to_string());
                self.vod_info = Some(vod_info.clone());

                // Helper function to wrap text
                let wrap_text = |text: &str, width: usize, indent: &str| -> Vec<String> {
                    let mut wrapped = Vec::new();
                    let words = text.split_whitespace();
                    let mut current_line = String::new();

                    for word in words {
                        if current_line.len() + word.len() + 1 > width {
                            if !current_line.is_empty() {
                                wrapped.push(format!("{}{}", indent, current_line));
                                current_line = word.to_string();
                            }
                        } else {
                            if !current_line.is_empty() {
                                current_line.push(' ');
                            }
                            current_line.push_str(word);
                        }
                    }
                    if !current_line.is_empty() {
                        wrapped.push(format!("{}{}", indent, current_line));
                    }
                    wrapped
                };

                // Create info display items
                let mut items = vec![format!("{}", vod_info.info.name), String::new()];

                if let Some(ref plot) = vod_info.info.plot {
                    let plot_trimmed = plot.trim();
                    if !plot_trimmed.is_empty() {
                        items.push("Description:".to_string());
                        items.extend(wrap_text(plot_trimmed, 75, "   "));
                        items.push(String::new());
                    }
                }

                if let Some(ref genre) = vod_info.info.genre
                    && !genre.trim().is_empty()
                {
                    items.push(format!("Genre: {}", genre));
                }

                if let Some(ref release_date) = vod_info.info.releasedate
                    && !release_date.trim().is_empty()
                {
                    items.push(format!("Release: {}", release_date));
                }

                if let Some(ref rating) = vod_info.info.rating
                    && !rating.trim().is_empty()
                {
                    items.push(format!("Rating: {}", rating));
                }

                if let Some(ref duration) = vod_info.info.duration
                    && !duration.trim().is_empty()
                {
                    items.push(format!("Duration: {}", duration));
                }

                if let Some(ref cast) = vod_info.info.cast {
                    let cast_trimmed = cast.trim();
                    if !cast_trimmed.is_empty() {
                        items.push("Cast:".to_string());
                        items.extend(wrap_text(cast_trimmed, 75, "   "));
                    }
                }

                if let Some(ref director) = vod_info.info.director {
                    let director_trimmed = director.trim();
                    if !director_trimmed.is_empty() {
                        // Wrap director if it's too long
                        if director_trimmed.len() > 60 {
                            items.push("Director:".to_string());
                            items.extend(wrap_text(director_trimmed, 75, "   "));
                        } else {
                            items.push(format!("Director: {}", director_trimmed));
                        }
                    }
                }

                items.push(String::new());
                items.push(format!(
                    "Format: {}",
                    vod_info.movie_data.container_extension
                ));

                // Add stream URL (wrapped if needed)
                let extension = Some(vod_info.movie_data.container_extension.as_str());
                let url = if let Some(api) = &self.current_api {
                    api.get_stream_url(stream.stream_id, "movie", extension)
                } else {
                    String::new()
                };
                items.push(String::new());
                items.push("Stream URL:".to_string());
                if url.len() > 75 {
                    // Break long URLs at logical points
                    let mut url_line = String::from("   ");
                    for (i, ch) in url.chars().enumerate() {
                        url_line.push(ch);
                        if (i > 0 && i % 70 == 0) || (ch == '&' && url_line.len() > 40) {
                            items.push(url_line.clone());
                            url_line = String::from("   ");
                        }
                    }
                    if url_line.len() > 3 {
                        items.push(url_line);
                    }
                } else {
                    items.push(format!("   {}", url));
                }

                // Add menu options
                items.push(String::new());
                items.push("─────────────────────────────────────".to_string());
                items.push(String::new());
                items.push("Actions:".to_string());
                items.push(String::new());
                items.push("  > Play Movie".to_string());
                items.push("  > Copy URL to Logs".to_string());
                items.push("  > Back to Movies".to_string());

                self.items = items;
                self.reset_filter();
                self.state = AppState::VodInfo(VodInfoState {
                    stream,
                    saved_filter,
                    saved_selected,
                    saved_filtered_indices,
                    saved_scroll,
                    saved_items,
                    content_scroll: 0, // Start with content at top
                });
                // Start with "Play Movie" selected
                self.selected_index = self
                    .items
                    .iter()
                    .position(|s| s.contains("Play Movie"))
                    .unwrap_or(0);
                self.scroll_offset = 0;

                self.add_log(format!("Ready to play: {}", vod_info.info.name));
                self.add_log(
                    "Use ↑↓ to navigate menu, PgUp/PgDn/Space/Shift+Space to scroll content"
                        .to_string(),
                );
            }
            Err(e) => {
                self.add_log(format!("Failed to load VOD info: {}", e));
                self.add_log("Falling back to direct playback...".to_string());
                // Fallback to direct play
                self.play_stream(&stream).await;
            }
        }
    }

    async fn play_vod_stream(&mut self, stream: &Stream) {
        // Store the current state to return to after starting playback
        let return_state = self.state.clone();

        self.add_log(format!("Playing: {}", stream.name));

        if let Some(api) = &self.current_api {
            // Use the container extension from VOD info if available
            let extension = self
                .vod_info
                .as_ref()
                .map(|info| info.movie_data.container_extension.as_str());

            let url = api.get_stream_url(stream.stream_id, "movie", extension);

            // Log the stream URL
            self.add_log(format!("Stream URL: {}", url));

            // Use TUI-specific play method that runs in background
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play movie: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Player started in background window".to_string());
                self.add_log("Continue browsing while video plays".to_string());
                // Return to the VOD info state so user can see the info
                self.state = return_state;
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

    fn start_search(&mut self) {
        self.search_active = true;
        self.search_query.clear();
        self.apply_filter();
        self.status_message =
            Some("Search: Type to filter, Enter to confirm, Esc to cancel".to_string());
    }

    fn update_search(&mut self, c: char) {
        if self.search_active {
            self.search_query.push(c);
            self.apply_filter();
            self.status_message = Some(format!("Search: {}", self.search_query));
        }
    }

    fn delete_search_char(&mut self) {
        if self.search_active && !self.search_query.is_empty() {
            self.search_query.pop();
            self.apply_filter();
            self.status_message = Some(if self.search_query.is_empty() {
                "Search: Type to filter, Enter to confirm, Esc to cancel".to_string()
            } else {
                format!("Search: {}", self.search_query)
            });
        }
    }

    fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            // Case-insensitive substring search
            let query_lower = self.search_query.to_lowercase();
            self.filtered_indices = self
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.to_lowercase().contains(&query_lower))
                .map(|(idx, _)| idx)
                .collect();
        }

        // Reset selection to first filtered item
        if !self.filtered_indices.is_empty() {
            self.selected_index = self.filtered_indices[0];
            self.scroll_offset = 0;
        }
    }

    fn cancel_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.filtered_indices = (0..self.items.len()).collect();
        self.status_message = None;
    }

    fn confirm_search(&mut self) {
        self.search_active = false;
        // Keep the filter applied
        self.status_message = if !self.search_query.is_empty() {
            Some(format!(
                "Filtered: \"{}\" (Press '/' to search again)",
                self.search_query
            ))
        } else {
            None
        };
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

    fn ensure_selected_visible(&mut self) {
        // Make sure the selected item is visible on screen
        let visible_height = self.page_size;

        // If selected item is above visible area, scroll up
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        }
        // If selected item is below visible area, scroll down
        else if self.selected_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected_index.saturating_sub(visible_height - 1);
        }
    }

    fn reset_filter(&mut self) {
        self.search_query.clear();
        self.search_active = false;
        self.filtered_indices = (0..self.items.len()).collect();
        self.status_message = None;
    }
}
