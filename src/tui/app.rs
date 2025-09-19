// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

// ContentType moved to commands module, but we'll define it locally for now
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentType {
    Live,
    Movies,
    Series,
}

impl ContentType {}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentType::Live => write!(f, "Live TV"),
            ContentType::Movies => write!(f, "Movies"),
            ContentType::Series => write!(f, "TV Series"),
        }
    }
}
use crate::config::ProviderConfig;
use crate::ignore::IgnoreConfig;
use crate::player::Player;
use crate::xtream::{ApiEpisode, Category, FavouriteStream, Stream, VodInfoResponse, XTreamAPI};
use chrono::{DateTime, Local};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

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

use crate::config::PlayMode;

impl std::fmt::Display for PlayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayMode::Mpv => write!(f, "MPV"),
            PlayMode::MpvInTerminal => write!(f, "MPV in Terminal"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NavigationState {
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub search_query: String,
    pub filtered_indices: Vec<usize>,
}

impl NavigationState {
    pub fn new() -> Self {
        Self::default()
    }
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
    CrossProviderFavourites,
    StreamAdvancedMenu(Stream, ContentType),
    Configuration,
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
    CacheRefresh, // Exit TUI temporarily to refresh cache
}

pub struct App {
    pub state: AppState,
    pub config: crate::config::Config,
    pub current_api: Option<XTreamAPI>,
    pub current_provider_name: Option<String>,
    pub player: Player,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub items: Vec<String>,
    pub status_message: Option<String>,
    pub progress: Option<(f64, String)>,
    pub logs: Vec<(DateTime<Local>, String)>,
    pub show_help: bool,
    pub help_scroll_offset: usize,
    pub log_display_mode: LogDisplayMode,
    pub log_selected_index: usize,
    pub log_scroll_offset: usize,
    pub visible_height: usize, // Dynamically calculated based on terminal size
    pub search_query: String,
    pub search_active: bool,
    pub filtered_indices: Vec<usize>,
    pub config_state: NavigationState,
    categories: Vec<Category>,
    streams: Vec<Stream>,
    seasons: Vec<TuiSeason>,
    episodes: Vec<ApiEpisode>,
    cross_provider_favourites: Vec<(FavouriteStream, ProviderConfig)>,
    vod_info: Option<VodInfoResponse>,
    // Cache for categories by content type
    cached_categories: HashMap<ContentType, Vec<Category>>,
    // Cache for streams by content type and category ID
    cached_streams: HashMap<(ContentType, String), Vec<Stream>>,
    // Navigation state history for preserving selections when going back
    provider_selection_state: NavigationState,
    main_menu_state: NavigationState,
    category_selection_states: HashMap<ContentType, NavigationState>,
    stream_selection_states: HashMap<(ContentType, String), NavigationState>,
    season_selection_state: NavigationState,
    cross_provider_favourites_state: NavigationState,
    ignore_config: IgnoreConfig,
    previous_state_before_menu: Option<Box<AppState>>,
    previous_items_before_menu: Vec<String>,
    previous_nav_before_menu: NavigationState,
}

impl App {
    pub fn update_visible_height(&mut self, height: usize) {
        // Update the visible height based on terminal size
        // Account for header (3 lines) and footer (1 line)
        self.visible_height = height.saturating_sub(4).max(1);
    }

    pub fn new(config: crate::config::Config, player: Player) -> Self {
        let providers = config.providers.clone();
        let items = if providers.len() > 1 {
            let mut items = vec!["Favourites".to_string()];
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
            config,
            current_api: None,
            current_provider_name: None,
            player,
            selected_index: 0,
            scroll_offset: 0,
            items,
            status_message: None,
            progress: None,
            logs: Vec::new(),
            show_help: false,
            help_scroll_offset: 0,
            log_display_mode: LogDisplayMode::Side,
            log_selected_index: 0,
            log_scroll_offset: 0,
            visible_height: 20, // Will be updated on first render
            search_query: String::new(),
            search_active: false,
            filtered_indices,
            config_state: NavigationState::new(),
            categories: Vec::new(),
            streams: Vec::new(),
            seasons: Vec::new(),
            episodes: Vec::new(),
            cross_provider_favourites: Vec::new(),
            vod_info: None,
            cached_categories: HashMap::new(),
            cached_streams: HashMap::new(),
            provider_selection_state: NavigationState::new(),
            main_menu_state: NavigationState::new(),
            category_selection_states: HashMap::new(),
            stream_selection_states: HashMap::new(),
            season_selection_state: NavigationState::new(),
            cross_provider_favourites_state: NavigationState::new(),
            ignore_config: IgnoreConfig::load().unwrap_or_default(),
            previous_state_before_menu: None,
            previous_items_before_menu: Vec::new(),
            previous_nav_before_menu: NavigationState::new(),
        }
    }

    pub fn tick(&mut self) {
        // Update any time-based UI elements here
        // Note: Player status check moved to async tick method in run_app
    }

    pub async fn async_tick(&mut self) {
        // Auto-connect to single provider on startup
        if matches!(self.state, AppState::Loading(_))
            && self.config.providers.len() == 1
            && self.current_api.is_none()
        {
            let provider = self.config.providers[0].clone();
            self.connect_to_provider(&provider).await;
        }

        // Check player status periodically to detect exits
        if matches!(self.state, AppState::Playing(_)) {
            let (is_running, exit_message) = self.player.check_player_status().await;

            if !is_running {
                // Return to main menu when player exits
                self.state = AppState::MainMenu;
                self.restore_navigation_state(&AppState::MainMenu);
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
                    let page_size = self.visible_height.saturating_sub(2).max(1);
                    self.log_selected_index = self.log_selected_index.saturating_sub(page_size);
                    if self.log_selected_index < self.log_scroll_offset {
                        self.log_scroll_offset = self.log_selected_index;
                    }
                    return None;
                }
                KeyCode::PageDown => {
                    let page_size = self.visible_height.saturating_sub(2).max(1);
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

        // If help is shown, handle help-specific navigation
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::F(1) | KeyCode::Esc => {
                    self.show_help = false;
                    self.help_scroll_offset = 0; // Reset scroll when closing
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.help_scroll_offset > 0 {
                        self.help_scroll_offset -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    // We'll need to pass the total lines from the widget
                    self.help_scroll_offset += 1;
                }
                KeyCode::PageUp => {
                    self.help_scroll_offset = self.help_scroll_offset.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    self.help_scroll_offset += 10;
                }
                KeyCode::Home => {
                    self.help_scroll_offset = 0;
                }
                KeyCode::End => {
                    // Set to a high value, will be clamped in rendering
                    self.help_scroll_offset = 1000;
                }
                _ => {
                    // Any other key closes the help
                    self.show_help = false;
                    self.help_scroll_offset = 0;
                }
            }
            return None;
        }

        if key.code == KeyCode::Char('q') {
            return Some(Action::Quit);
        }

        if key.code == KeyCode::Char('?') || key.code == KeyCode::F(1) {
            self.show_help = true;
            self.help_scroll_offset = 0; // Reset scroll when opening
            return None;
        }

        match self.state.clone() {
            AppState::Error(_) => {
                if key.code == KeyCode::Enter || key.code == KeyCode::Esc {
                    // Return to provider selection if no provider is connected
                    if self.current_api.is_none() {
                        self.state = AppState::ProviderSelection;
                        self.restore_navigation_state(&AppState::ProviderSelection);
                    } else {
                        self.state = AppState::MainMenu;
                        self.restore_navigation_state(&AppState::MainMenu);
                        self.update_main_menu_items();
                    }
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
                        // Favourites selected
                        self.save_current_navigation_state();
                        self.load_all_favourites().await;
                    } else if self.selected_index - 1 < self.config.providers.len() {
                        let provider = self.config.providers[self.selected_index - 1].clone();
                        self.save_current_navigation_state();
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
                    self.save_current_navigation_state();
                    if let Some(action) = self.handle_main_menu_selection().await {
                        return Some(action);
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    if self.config.providers.len() > 1 {
                        self.save_current_navigation_state();
                        self.state = AppState::ProviderSelection;
                        self.restore_navigation_state(&AppState::ProviderSelection);
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
                KeyCode::Char('i') => {
                    // Toggle ignore for current category
                    if let Some(category) = self.get_current_category() {
                        if category.category_name != "All" && category.category_id != "all" {
                            // Don't allow ignoring "All" category
                            let _provider_name = self
                                .current_provider_name
                                .as_ref()
                                .unwrap_or(&String::new())
                                .clone();

                            self.add_log(format!(
                                "Toggling ignore for category '{}'",
                                category.category_name
                            ));

                            match self.ignore_config.toggle_category(&category.category_name) {
                                Ok(is_ignored) => {
                                    let msg = if is_ignored {
                                        format!(
                                            "Category '{}' will be hidden",
                                            category.category_name
                                        )
                                    } else {
                                        format!(
                                            "Category '{}' will be shown",
                                            category.category_name
                                        )
                                    };
                                    self.add_log(msg.clone());
                                    self.status_message = Some(msg);

                                    // Save current state before reloading
                                    let current_filter_pos = self
                                        .filtered_indices
                                        .iter()
                                        .position(|&idx| idx == self.selected_index)
                                        .unwrap_or(0);
                                    let current_scroll = self.scroll_offset;

                                    // Find the first visible item that won't be ignored (for scroll anchoring)
                                    let visible_anchor =
                                        self.filtered_indices.iter().skip(current_scroll).find_map(
                                            |&idx| {
                                                let cat = &self.categories[idx];
                                                if cat.category_name != category.category_name {
                                                    Some(cat.category_name.clone())
                                                } else {
                                                    None
                                                }
                                            },
                                        );

                                    // For determining next selection: get the next item in the filtered list
                                    let next_item_name = if is_ignored {
                                        // Check if we're at the last position
                                        let is_last_item =
                                            current_filter_pos == self.filtered_indices.len() - 1;

                                        if is_last_item && current_filter_pos > 0 {
                                            // If at the last item and not at index 0, prefer the previous item
                                            self.filtered_indices
                                                .iter()
                                                .take(current_filter_pos)
                                                .rev()
                                                .find_map(|&idx| {
                                                    let cat = &self.categories[idx];
                                                    if cat.category_name != category.category_name {
                                                        Some(cat.category_name.clone())
                                                    } else {
                                                        None
                                                    }
                                                })
                                        } else {
                                            // Otherwise, look for the next item (forward, then wrap)
                                            self.filtered_indices
                                                .iter()
                                                .skip(current_filter_pos + 1)
                                                .chain(
                                                    self.filtered_indices
                                                        .iter()
                                                        .take(current_filter_pos),
                                                )
                                                .find_map(|&idx| {
                                                    let cat = &self.categories[idx];
                                                    if cat.category_name != category.category_name {
                                                        Some(cat.category_name.clone())
                                                    } else {
                                                        None
                                                    }
                                                })
                                        }
                                    } else {
                                        None
                                    };

                                    // Reload categories without restoring navigation state
                                    // (preserves filter)
                                    self.load_categories_without_nav_restore(content_type).await;

                                    // Adjust selection and scroll after reload
                                    if !self.filtered_indices.is_empty() {
                                        // First, try to restore scroll position using the anchor
                                        if let Some(anchor_name) = visible_anchor {
                                            if let Some(anchor_pos) =
                                                self.filtered_indices.iter().position(|&idx| {
                                                    self.categories[idx].category_name
                                                        == anchor_name
                                                })
                                            {
                                                // Try to keep the anchor item at the same visual position
                                                self.scroll_offset = anchor_pos;
                                            } else {
                                                // Anchor not found, try to maintain scroll position
                                                self.scroll_offset = current_scroll.min(
                                                    self.filtered_indices
                                                        .len()
                                                        .saturating_sub(self.visible_height),
                                                );
                                            }
                                        } else {
                                            // No anchor, maintain scroll position as best as possible
                                            self.scroll_offset = current_scroll.min(
                                                self.filtered_indices
                                                    .len()
                                                    .saturating_sub(self.visible_height),
                                            );
                                        }

                                        // Now select the appropriate item
                                        let new_selected = if let Some(next_name) = next_item_name {
                                            // Find the item we want to select
                                            self.filtered_indices
                                                .iter()
                                                .find(|&&idx| {
                                                    self.categories[idx].category_name == next_name
                                                })
                                                .copied()
                                                .unwrap_or_else(|| {
                                                    // Fallback: select first visible item
                                                    let pos = self.scroll_offset.min(
                                                        self.filtered_indices
                                                            .len()
                                                            .saturating_sub(1),
                                                    );
                                                    self.filtered_indices[pos]
                                                })
                                        } else {
                                            // Not ignoring: try to maintain position
                                            let pos = current_filter_pos
                                                .min(self.filtered_indices.len().saturating_sub(1));
                                            self.filtered_indices[pos]
                                        };

                                        self.selected_index = new_selected;

                                        // Only adjust scroll if selected item is not visible
                                        if let Some(filter_pos) = self
                                            .filtered_indices
                                            .iter()
                                            .position(|&idx| idx == new_selected)
                                        {
                                            if filter_pos < self.scroll_offset {
                                                self.scroll_offset = filter_pos;
                                            } else if filter_pos
                                                >= self.scroll_offset + self.visible_height
                                            {
                                                self.scroll_offset = filter_pos.saturating_sub(
                                                    self.visible_height.saturating_sub(1),
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    let msg = format!("Failed to toggle ignore: {}", e);
                                    self.add_log(msg.clone());
                                    self.status_message = Some(msg);
                                }
                            }
                        } else {
                            self.status_message = Some("Cannot ignore 'All' category".to_string());
                        }
                    } else {
                        self.add_log("No category selected".to_string());
                    }
                }
                KeyCode::End | KeyCode::Char('G') => self.move_selection_end(),
                KeyCode::Char('r') => {
                    // Force refresh categories
                    let ct = content_type;
                    self.add_log("Refreshing categories...".to_string());
                    self.load_categories_internal(ct, true, true).await;
                }
                KeyCode::Enter => {
                    if self.selected_index < self.categories.len() {
                        let category = self.categories[self.selected_index].clone();
                        self.save_current_navigation_state();
                        self.load_streams(content_type, category).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // If there's an active filter, clear it instead of going back
                    if !self.search_query.is_empty() {
                        self.reset_filter();
                    } else {
                        self.save_current_navigation_state();
                        self.state = AppState::MainMenu;
                        self.restore_navigation_state(&AppState::MainMenu);
                        self.update_main_menu_items();
                    }
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
                    let ct = content_type;
                    let cat = category.clone();
                    self.add_log("Refreshing streams...".to_string());
                    self.load_streams_internal(ct, cat, true, true).await;
                }
                KeyCode::Char('f') => {
                    // selected_index already points to the correct stream
                    if self.selected_index < self.streams.len() {
                        let stream = self.streams[self.selected_index].clone();
                        self.toggle_favourite_stream(&stream).await;
                    }
                }
                KeyCode::Char('i') => {
                    // Toggle ignore for current channel (only for live TV)
                    if content_type == ContentType::Live && self.selected_index < self.streams.len()
                    {
                        let stream_name = self.streams[self.selected_index].name.clone();
                        self.add_log(format!("Toggling ignore for channel '{}'", stream_name));
                        match self.ignore_config.toggle_channel(&stream_name) {
                            Ok(is_ignored) => {
                                let msg = if is_ignored {
                                    format!("Channel '{}' will be hidden", stream_name)
                                } else {
                                    format!("Channel '{}' will be shown", stream_name)
                                };
                                self.add_log(msg.clone());
                                self.status_message = Some(msg);

                                // Save current state before reloading
                                let current_filter_pos = self
                                    .filtered_indices
                                    .iter()
                                    .position(|&idx| idx == self.selected_index)
                                    .unwrap_or(0);
                                let current_scroll = self.scroll_offset;

                                // Find the first visible item that won't be ignored (for scroll anchoring)
                                let visible_anchor =
                                    self.filtered_indices.iter().skip(current_scroll).find_map(
                                        |&idx| {
                                            let strm = &self.streams[idx];
                                            if strm.name != stream_name {
                                                Some(strm.name.clone())
                                            } else {
                                                None
                                            }
                                        },
                                    );

                                // For determining next selection: get the next item in the filtered list
                                let next_stream_name = if is_ignored {
                                    // Check if we're at the last position
                                    let is_last_item =
                                        current_filter_pos == self.filtered_indices.len() - 1;

                                    if is_last_item && current_filter_pos > 0 {
                                        // If at the last item and not at index 0, prefer the previous item
                                        self.filtered_indices
                                            .iter()
                                            .take(current_filter_pos)
                                            .rev()
                                            .find_map(|&idx| {
                                                let strm = &self.streams[idx];
                                                if strm.name != stream_name {
                                                    Some(strm.name.clone())
                                                } else {
                                                    None
                                                }
                                            })
                                    } else {
                                        // Otherwise, look for the next item (forward, then wrap)
                                        self.filtered_indices
                                            .iter()
                                            .skip(current_filter_pos + 1)
                                            .chain(
                                                self.filtered_indices
                                                    .iter()
                                                    .take(current_filter_pos),
                                            )
                                            .find_map(|&idx| {
                                                let strm = &self.streams[idx];
                                                if strm.name != stream_name {
                                                    Some(strm.name.clone())
                                                } else {
                                                    None
                                                }
                                            })
                                    }
                                } else {
                                    None
                                };

                                // Reload streams to apply the change (preserves filter)
                                let ct = content_type;
                                let cat = category.clone();
                                self.load_streams_without_nav_restore(ct, cat).await;

                                // Adjust selection and scroll after reload
                                if !self.filtered_indices.is_empty() {
                                    // First, try to restore scroll position using the anchor
                                    if let Some(anchor_name) = visible_anchor {
                                        if let Some(anchor_pos) = self
                                            .filtered_indices
                                            .iter()
                                            .position(|&idx| self.streams[idx].name == anchor_name)
                                        {
                                            // Try to keep the anchor item at the same visual position
                                            self.scroll_offset = anchor_pos;
                                        } else {
                                            // Anchor not found, try to maintain scroll position
                                            self.scroll_offset = current_scroll.min(
                                                self.filtered_indices
                                                    .len()
                                                    .saturating_sub(self.visible_height),
                                            );
                                        }
                                    } else {
                                        // No anchor, maintain scroll position as best as possible
                                        self.scroll_offset = current_scroll.min(
                                            self.filtered_indices
                                                .len()
                                                .saturating_sub(self.visible_height),
                                        );
                                    }

                                    // Now select the appropriate item
                                    let new_selected = if let Some(next_name) = next_stream_name {
                                        // Find the stream we want to select
                                        self.filtered_indices
                                            .iter()
                                            .find(|&&idx| self.streams[idx].name == next_name)
                                            .copied()
                                            .unwrap_or_else(|| {
                                                // Fallback: select first visible item
                                                let pos = self.scroll_offset.min(
                                                    self.filtered_indices.len().saturating_sub(1),
                                                );
                                                self.filtered_indices[pos]
                                            })
                                    } else {
                                        // Not ignoring: try to maintain position
                                        let pos = current_filter_pos
                                            .min(self.filtered_indices.len().saturating_sub(1));
                                        self.filtered_indices[pos]
                                    };

                                    self.selected_index = new_selected;

                                    // Only adjust scroll if selected item is not visible
                                    if let Some(filter_pos) = self
                                        .filtered_indices
                                        .iter()
                                        .position(|&idx| idx == new_selected)
                                    {
                                        if filter_pos < self.scroll_offset {
                                            self.scroll_offset = filter_pos;
                                        } else if filter_pos
                                            >= self.scroll_offset + self.visible_height
                                        {
                                            self.scroll_offset = filter_pos.saturating_sub(
                                                self.visible_height.saturating_sub(1),
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                self.add_log(format!("Failed to toggle ignore: {}", e));
                                self.status_message =
                                    Some(format!("Failed to toggle ignore: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Enter => {
                    // selected_index already points to the correct stream
                    if self.selected_index < self.streams.len() {
                        let stream = self.streams[self.selected_index].clone();
                        match content_type {
                            ContentType::Series => {
                                self.save_current_navigation_state();
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
                KeyCode::Char('a') => {
                    // Show advanced menu for live streams
                    if content_type == ContentType::Live && self.selected_index < self.streams.len()
                    {
                        let stream = self.streams[self.selected_index].clone();
                        self.show_stream_advanced_menu(stream, content_type).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // If there's an active filter, clear it instead of going back
                    if !self.search_query.is_empty() {
                        self.reset_filter();
                    } else {
                        // Go back to category selection
                        self.save_current_navigation_state();
                        self.state = AppState::CategorySelection(content_type);
                        self.restore_navigation_state(&AppState::CategorySelection(content_type));
                        // Reload categories to ensure UI is in sync
                        self.load_categories(content_type).await;
                    }
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
                                || item.contains("Play in Detached")
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
                                || item.contains("Play in Detached")
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
                        let visible_height = self.visible_height.saturating_sub(2).max(1);
                        state.content_scroll = state.content_scroll.saturating_sub(visible_height);
                    }
                }
                KeyCode::PageDown => {
                    // Always scroll content down by page
                    if let AppState::VodInfo(state) = &mut self.state {
                        let visible_height = self.visible_height.saturating_sub(2).max(1);
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
                            let visible_height = self.visible_height.saturating_sub(2).max(1);
                            state.content_scroll =
                                state.content_scroll.saturating_sub(visible_height);
                        }
                    } else {
                        // Space - scroll down by page
                        if let AppState::VodInfo(state) = &mut self.state {
                            let visible_height = self.visible_height.saturating_sub(2).max(1);
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
                        let visible_height = self.visible_height.saturating_sub(2).max(1);
                        state.content_scroll = self
                            .items
                            .len()
                            .saturating_sub(visible_height.min(self.items.len()));
                    }
                }
                KeyCode::Enter => {
                    // Always execute selected menu action
                    let selected_item = &self.items[self.selected_index];

                    if selected_item.contains("Play Movie") && !selected_item.contains("Detached") {
                        self.play_vod_stream(&vod_state.stream.clone()).await;
                    } else if selected_item.contains("Play in Detached Window") {
                        self.play_vod_stream_detached(&vod_state.stream.clone())
                            .await;
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
                        let season = self.seasons[self.selected_index].clone();
                        self.save_current_navigation_state();
                        self.load_episodes(series.clone(), season).await;
                    }
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // Go back to stream selection
                    self.save_current_navigation_state();
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

                    self.state = AppState::StreamSelection(ContentType::Series, category.clone());
                    self.restore_navigation_state(&AppState::StreamSelection(
                        ContentType::Series,
                        category.clone(),
                    ));
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
                    self.save_current_navigation_state();
                    self.state = AppState::SeasonSelection(series.clone());
                    self.restore_navigation_state(&AppState::SeasonSelection(series.clone()));
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
                                != crate::XTreamAPI::new_with_id(
                                    provider.url.clone(),
                                    provider.username.clone(),
                                    provider.password.clone(),
                                    provider.name.clone(),
                                    provider.id.clone(),
                                )
                                .unwrap()
                                .provider_hash
                        {
                            self.add_log(format!(
                                "Connecting to provider: {}",
                                provider.name.as_ref().unwrap_or(&provider.url)
                            ));

                            match crate::XTreamAPI::new_with_id(
                                provider.url.clone(),
                                provider.username.clone(),
                                provider.password.clone(),
                                provider.name.clone(),
                                provider.id.clone(),
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
                            // Use .ts extension if configured for live streams
                            let extension = if favourite.stream_type == "live"
                                && self.config.settings.use_ts_for_live
                            {
                                Some("ts")
                            } else {
                                None
                            };

                            let stream_url = api.get_stream_url(
                                favourite.stream_id,
                                &favourite.stream_type,
                                extension,
                            );

                            self.add_log(format!("Playing: {}", favourite.name));

                            // Log the stream URL to the logs panel
                            self.add_log(format!("Stream URL: {}", stream_url));

                            // Use play mode from configuration
                            let result = match self.config.settings.play_mode {
                                PlayMode::Mpv => self.player.play_tui(&stream_url).await,
                                PlayMode::MpvInTerminal => {
                                    self.player.play_in_terminal(&stream_url).await
                                }
                            };

                            if let Err(e) = result {
                                self.state =
                                    AppState::Error(format!("Failed to play favourite: {}", e));
                                self.add_log(format!("Playback failed: {}", e));
                            } else {
                                match self.config.settings.play_mode {
                                    PlayMode::Mpv => {
                                        self.add_log(
                                            "Player started in background window".to_string(),
                                        );
                                        self.add_log(
                                            "Continue browsing while video plays".to_string(),
                                        );
                                    }
                                    PlayMode::MpvInTerminal => {
                                        self.add_log("Player started in terminal mode".to_string());
                                    }
                                }
                                // Stay in CrossProviderFavourites state
                            }
                        }
                    }
                }
                KeyCode::Char('a') => {
                    // Show advanced menu for live streams in favorites
                    if self.selected_index < self.cross_provider_favourites.len() {
                        let (favourite, provider) =
                            self.cross_provider_favourites[self.selected_index].clone();

                        // Only show advanced menu for live streams
                        if favourite.stream_type == "live" {
                            // Connect to provider if needed
                            if self.current_api.is_none()
                                || self.current_api.as_ref().unwrap().provider_hash
                                    != crate::XTreamAPI::new_with_id(
                                        provider.url.clone(),
                                        provider.username.clone(),
                                        provider.password.clone(),
                                        provider.name.clone(),
                                        provider.id.clone(),
                                    )
                                    .unwrap()
                                    .provider_hash
                            {
                                match crate::XTreamAPI::new_with_id(
                                    provider.url.clone(),
                                    provider.username.clone(),
                                    provider.password.clone(),
                                    provider.name.clone(),
                                    provider.id.clone(),
                                ) {
                                    Ok(mut api) => {
                                        api.disable_progress();
                                        self.current_api = Some(api);
                                        self.current_provider_name = provider.name.clone();
                                    }
                                    Err(e) => {
                                        self.add_log(format!(
                                            "Failed to connect to provider: {}",
                                            e
                                        ));
                                        return None;
                                    }
                                }
                            }

                            // Convert favourite to Stream
                            let stream = Stream {
                                num: 0,
                                name: favourite.name.clone(),
                                stream_type: favourite.stream_type.clone(),
                                stream_id: favourite.stream_id,
                                stream_icon: None,
                                epg_channel_id: None,
                                added: None,
                                category_id: favourite.category_id,
                                category_ids: None,
                                custom_sid: None,
                                tv_archive: None,
                                direct_source: None,
                                tv_archive_duration: None,
                                is_adult: None,
                                rating: None,
                                rating_5based: None,
                                container_extension: Some("m3u8".to_string()),
                            };

                            self.show_stream_advanced_menu(stream, ContentType::Live)
                                .await;
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

                        let api = match crate::XTreamAPI::new_with_id(
                            provider.url.clone(),
                            provider.username.clone(),
                            provider.password.clone(),
                            provider.name.clone(),
                            provider.id.clone(),
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
                    // If there's an active filter, clear it instead of going back
                    if !self.search_query.is_empty() {
                        self.reset_filter();
                    } else {
                        self.save_current_navigation_state();
                        // Go back to MainMenu for single provider, ProviderSelection for multiple
                        if self.config.providers.len() == 1 {
                            self.state = AppState::MainMenu;
                            self.restore_navigation_state(&AppState::MainMenu);
                            self.update_main_menu_items();
                        } else {
                            self.state = AppState::ProviderSelection;
                            self.restore_navigation_state(&AppState::ProviderSelection);
                            self.update_provider_items();
                        }
                    }
                }
                KeyCode::Char('/') => {
                    self.search_active = true;
                    self.search_query.clear();
                }
                _ => {}
            },
            AppState::Playing(_name) => match key.code {
                KeyCode::Esc | KeyCode::Char('s') => {
                    self.stop_playing();
                }
                _ => {}
            },
            AppState::StreamAdvancedMenu(stream, content_type) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::Enter => {
                    self.handle_stream_advanced_menu_selection(stream, content_type)
                        .await;
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // Go back to stream selection
                    self.restore_previous_state();
                }
                _ => {}
            },
            AppState::Configuration => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_selection_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_selection_down(),
                KeyCode::Enter => {
                    self.handle_configuration_selection();
                }
                KeyCode::Esc | KeyCode::Char('b') => {
                    // Go back to main menu
                    self.save_current_navigation_state();
                    self.state = AppState::MainMenu;
                    self.restore_navigation_state(&AppState::MainMenu);
                    self.update_main_menu_items();
                }
                _ => {}
            },
            _ => {}
        }

        None
    }

    fn move_selection_up(&mut self) {
        let indices = self.filtered_indices.clone();

        if indices.is_empty() {
            return;
        }

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            if current_pos > 0 {
                // Normal upward movement
                self.selected_index = indices[current_pos - 1];
            } else {
                // Wrap to bottom: move to last item
                self.selected_index = indices[indices.len() - 1];
            }
            self.ensure_selected_visible();
        }
    }

    fn move_selection_down(&mut self) {
        let indices = self.filtered_indices.clone();

        if indices.is_empty() {
            return;
        }

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            if current_pos < indices.len() - 1 {
                // Normal downward movement
                self.selected_index = indices[current_pos + 1];
            } else {
                // Wrap to top: move to first item
                self.selected_index = indices[0];
            }
            self.ensure_selected_visible();
        }
    }

    fn move_selection_page_up(&mut self) {
        let indices = self.filtered_indices.clone();

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            let page_size = self.visible_height.saturating_sub(2).max(1);
            let new_pos = current_pos.saturating_sub(page_size);
            self.selected_index = indices[new_pos];
            self.ensure_selected_visible();
        }
    }

    fn move_selection_page_down(&mut self) {
        let indices = self.filtered_indices.clone();

        if let Some(current_pos) = indices.iter().position(|&idx| idx == self.selected_index) {
            let page_size = self.visible_height.saturating_sub(2).max(1);
            let new_pos = (current_pos + page_size).min(indices.len() - 1);
            self.selected_index = indices[new_pos];
            self.ensure_selected_visible();
        }
    }

    fn move_selection_home(&mut self) {
        let indices = self.filtered_indices.clone();

        if !indices.is_empty() {
            self.selected_index = indices[0];
            self.scroll_offset = 0;
        }
    }

    fn move_selection_end(&mut self) {
        let indices = self.filtered_indices.clone();

        if !indices.is_empty() {
            self.selected_index = indices[indices.len() - 1];
            self.ensure_selected_visible();
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

        match XTreamAPI::new_with_id(
            provider.url.clone(),
            provider.username.clone(),
            provider.password.clone(),
            provider.name.clone(),
            provider.id.clone(),
        ) {
            Ok(mut api) => {
                // Set up logger for TUI mode
                api.disable_progress();
                // Note: We can't actually pass a closure that captures self here due to lifetime issues
                // Instead we'll just disable progress bars for now
                self.current_api = Some(api);
                self.current_provider_name = Some(
                    provider
                        .name
                        .clone()
                        .unwrap_or_else(|| provider.url.clone()),
                );
                // Clear caches when switching providers
                self.cached_categories.clear();
                self.cached_streams.clear();

                // Clear navigation states to prevent index out of bounds with different provider
                self.category_selection_states.clear();
                self.stream_selection_states.clear();
                self.season_selection_state = NavigationState::new();

                self.state = AppState::MainMenu;
                self.restore_navigation_state(&AppState::MainMenu);
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
        let mut items = vec!["Favourites".to_string()];
        items.extend(
            self.config
                .providers
                .iter()
                .map(|p| p.name.clone().unwrap_or_else(|| p.url.clone())),
        );
        self.items = items;
        self.reset_filter();
    }

    fn update_main_menu_items(&mut self) {
        let mut menu_items = vec![];

        // Only show Favourites in main menu when there's a single provider
        if self.config.providers.len() == 1 {
            menu_items.push("Favourites".to_string());
        }

        menu_items.extend(vec![
            "Live TV".to_string(),
            "Movies (VOD)".to_string(),
            "TV Series".to_string(),
            "Configuration".to_string(),
            "Refresh Cache".to_string(),
        ]);

        self.items = menu_items;
        self.reset_filter();
    }

    async fn handle_main_menu_selection(&mut self) -> Option<Action> {
        let has_single_provider = self.config.providers.len() == 1;

        // Adjust index based on whether Favourites is shown
        let adjusted_index = if has_single_provider {
            self.selected_index
        } else {
            // No Favourites option, so all indices shift down by 1
            self.selected_index + 1
        };

        match adjusted_index {
            0 if has_single_provider => {
                // Load all favourites for single provider
                self.load_all_favourites().await;
                None
            }
            1 => {
                self.load_categories(ContentType::Live).await;
                None
            }
            2 => {
                self.load_categories(ContentType::Movies).await;
                None
            }
            3 => {
                self.load_categories(ContentType::Series).await;
                None
            }
            4 => {
                self.show_configuration();
                None
            }
            5 => self.refresh_cache().await,
            _ => None,
        }
    }

    fn show_configuration(&mut self) {
        self.save_current_navigation_state();
        self.state = AppState::Configuration;
        self.update_configuration_items();
        self.restore_navigation_state(&AppState::Configuration);
    }

    fn update_configuration_items(&mut self) {
        self.items = vec![
            format!("Play Mode: {}", self.config.settings.play_mode),
            format!(
                "Use .ts URL for live streams: {}",
                if self.config.settings.use_ts_for_live {
                    "Enabled"
                } else {
                    "Disabled"
                }
            ),
            "Back".to_string(),
        ];
        self.reset_filter();
    }

    fn handle_configuration_selection(&mut self) {
        match self.selected_index {
            0 => {
                // Toggle play mode
                self.config.settings.play_mode = match self.config.settings.play_mode {
                    PlayMode::Mpv => PlayMode::MpvInTerminal,
                    PlayMode::MpvInTerminal => PlayMode::Mpv,
                };
                // Save configuration
                if let Some(path) = crate::config::Config::default_config_path() {
                    if let Err(e) = self.config.save(&path) {
                        self.add_log(format!("Failed to save settings: {}", e));
                    } else {
                        self.add_log("Settings saved".to_string());
                    }
                } else {
                    self.add_log("Failed to determine config path".to_string());
                }
                self.update_configuration_items();
            }
            1 => {
                // Toggle .ts URL preference
                self.config.settings.use_ts_for_live = !self.config.settings.use_ts_for_live;
                // Save configuration
                if let Some(path) = crate::config::Config::default_config_path() {
                    if let Err(e) = self.config.save(&path) {
                        self.add_log(format!("Failed to save settings: {}", e));
                    } else {
                        self.add_log("Settings saved".to_string());
                    }
                } else {
                    self.add_log("Failed to determine config path".to_string());
                }
                self.update_configuration_items();
            }
            2 => {
                // Back
                self.save_current_navigation_state();
                self.state = AppState::MainMenu;
                self.restore_navigation_state(&AppState::MainMenu);
                self.update_main_menu_items();
            }
            _ => {}
        }
    }

    async fn load_categories(&mut self, content_type: ContentType) {
        self.load_categories_internal(content_type, false, true)
            .await;
    }

    async fn load_categories_without_nav_restore(&mut self, content_type: ContentType) {
        // Save current filter state
        let saved_query = self.search_query.clone();
        let saved_search_active = self.search_active;

        self.load_categories_internal(content_type, false, false)
            .await;

        // Restore filter state and reapply
        self.search_query = saved_query;
        self.search_active = saved_search_active;
        self.apply_filter();
    }

    async fn load_categories_internal(
        &mut self,
        content_type: ContentType,
        force_refresh: bool,
        restore_nav: bool,
    ) {
        // Check cache first if not forcing refresh
        if !force_refresh && let Some(cached) = self.cached_categories.get(&content_type) {
            let ct = content_type;
            // Filter out ignored categories from cache
            let _provider_name = self
                .current_provider_name
                .as_ref()
                .unwrap_or(&String::new())
                .clone();
            self.categories = cached
                .iter()
                .filter(|cat| !self.ignore_config.is_category_ignored(&cat.category_name))
                .cloned()
                .collect();
            self.add_log(format!("Using cached {} categories", ct));
            self.items = self
                .categories
                .iter()
                .map(|c| c.category_name.clone())
                .collect();
            self.reset_filter();
            self.state = AppState::CategorySelection(content_type);
            if restore_nav {
                self.restore_navigation_state(&AppState::CategorySelection(content_type));
            }
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

                    // Store in cache (unfiltered)
                    self.cached_categories
                        .insert(content_type, categories.clone());

                    // Filter out ignored categories
                    let _provider_name = self
                        .current_provider_name
                        .as_ref()
                        .unwrap_or(&String::new())
                        .clone();
                    self.categories = categories
                        .into_iter()
                        .filter(|cat| !self.ignore_config.is_category_ignored(&cat.category_name))
                        .collect();

                    self.items = self
                        .categories
                        .iter()
                        .map(|c| c.category_name.clone())
                        .collect();
                    self.reset_filter();
                    self.state = AppState::CategorySelection(content_type);
                    if restore_nav {
                        self.restore_navigation_state(&AppState::CategorySelection(content_type));
                    }
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
        self.load_streams_internal(content_type, category, false, true)
            .await;
    }

    async fn load_streams_without_nav_restore(
        &mut self,
        content_type: ContentType,
        category: Category,
    ) {
        // Save current filter state
        let saved_query = self.search_query.clone();
        let saved_search_active = self.search_active;

        self.load_streams_internal(content_type, category, false, false)
            .await;

        // Restore filter state and reapply
        self.search_query = saved_query;
        self.search_active = saved_search_active;
        self.apply_filter();
    }

    async fn load_streams_internal(
        &mut self,
        content_type: ContentType,
        category: Category,
        force_refresh: bool,
        restore_nav: bool,
    ) {
        // Check cache first if not forcing refresh
        let cache_key = (content_type, category.category_id.clone());
        if !force_refresh && let Some(cached) = self.cached_streams.get(&cache_key) {
            let cat_name = category.category_name.clone();
            self.streams = cached.clone();

            // Filter out ignored channels for live TV
            if content_type == ContentType::Live {
                self.streams
                    .retain(|s| !self.ignore_config.is_channel_ignored(&s.name));
            }

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
            self.state = AppState::StreamSelection(content_type, category.clone());
            if restore_nav {
                self.restore_navigation_state(&AppState::StreamSelection(content_type, category));
            }
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
                ContentType::Series => {
                    // Fetch series
                    let series_result = api.get_series(category_id).await;

                    // If this is the "All" category, deduplicate and show categories
                    if category_id.is_none() {
                        // First get all categories to map category IDs to names
                        let categories = api.get_series_categories().await.unwrap_or_default();
                        let category_map: std::collections::HashMap<String, String> = categories
                            .into_iter()
                            .map(|c| (c.category_id, c.category_name))
                            .collect();

                        series_result.map(|series_infos| {
                            // Group series by series_id to collect all categories
                            let mut series_map: std::collections::HashMap<
                                u32,
                                (crate::xtream::SeriesInfo, Vec<String>),
                            > = std::collections::HashMap::new();

                            for info in series_infos {
                                let category_name = info
                                    .category_id
                                    .as_ref()
                                    .and_then(|id| category_map.get(id))
                                    .cloned()
                                    .unwrap_or_else(|| "Unknown".to_string());

                                series_map
                                    .entry(info.series_id)
                                    .and_modify(|(_, categories)| {
                                        if !categories.contains(&category_name) {
                                            categories.push(category_name.clone());
                                        }
                                    })
                                    .or_insert((info, vec![category_name]));
                            }

                            // Convert back to Stream objects with category info in the name
                            series_map
                                .into_iter()
                                .map(|(_, (info, categories))| {
                                    let categories_str = categories.join(", ");
                                    Stream {
                                        num: info.num,
                                        name: format!("{} [{}]", info.name, categories_str),
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
                                    }
                                })
                                .collect()
                        })
                    } else {
                        // Normal processing for specific category
                        series_result.map(|series_infos| {
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
                }
            };

            match result {
                Ok(streams) => {
                    // Store in cache
                    self.cached_streams.insert(cache_key, streams.clone());

                    self.streams = streams;

                    // Filter out ignored channels for live TV
                    if content_type == ContentType::Live {
                        self.streams
                            .retain(|s| !self.ignore_config.is_channel_ignored(&s.name));
                    }

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
                    self.state = AppState::StreamSelection(content_type, category.clone());
                    if restore_nav {
                        self.restore_navigation_state(&AppState::StreamSelection(
                            content_type,
                            category,
                        ));
                    }
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

                    self.state = AppState::SeasonSelection(series.clone());
                    self.restore_navigation_state(&AppState::SeasonSelection(series));
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

                            self.state = AppState::EpisodeSelection(series.clone(), season);
                            // Episodes are a new navigation level, so we start fresh
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
        self.current_provider_name = Some("Favourites".to_string());

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
        let providers = self.config.providers.clone();
        for provider in &providers {
            let api = match crate::XTreamAPI::new_with_id(
                provider.url.clone(),
                provider.username.clone(),
                provider.password.clone(),
                provider.name.clone(),
                provider.id.clone(),
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
        self.restore_navigation_state(&AppState::CrossProviderFavourites);

        self.add_log(format!("Loaded {} favourites", self.items.len()));
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

                // Clear cross-provider favourites cache to force reload
                self.cross_provider_favourites.clear();

                // Update the display to show the star is removed
                if let Some(item) = self.items.get_mut(self.selected_index)
                    && item.starts_with("[FAV] ")
                {
                    *item = item[6..].to_string(); // Remove the fav prefix
                }
            } else {
                // Add to favourites
                let favourite = crate::xtream::FavouriteStream {
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

                // Clear cross-provider favourites cache to force reload
                self.cross_provider_favourites.clear();

                // Update the display to show the star
                if let Some(item) = self.items.get_mut(self.selected_index)
                    && !item.starts_with("[FAV] ")
                {
                    *item = format!("[FAV] {}", item);
                }
            }
        }
    }

    async fn play_stream(&mut self, stream: &Stream) {
        // Store the current state to return to after starting playback
        let return_state = self.state.clone();

        self.add_log(format!("Playing: {}", stream.name));

        if let Some(api) = &self.current_api {
            // Use .ts extension if configured for live streams
            let extension = if stream.stream_type == "live" && self.config.settings.use_ts_for_live
            {
                Some("ts")
            } else {
                stream.container_extension.as_deref()
            };

            let url = api.get_stream_url(
                stream.stream_id,
                if stream.stream_type == "live" {
                    "live"
                } else {
                    "movie"
                },
                extension,
            );

            // Log the stream URL to the logs panel
            self.add_log(format!("Stream URL: {}", url));

            // Use play mode from configuration
            let result = match self.config.settings.play_mode {
                PlayMode::Mpv => self.player.play_tui(&url).await,
                PlayMode::MpvInTerminal => self.player.play_in_terminal(&url).await,
            };

            if let Err(e) = result {
                self.state = AppState::Error(format!("Failed to play stream: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                match self.config.settings.play_mode {
                    PlayMode::Mpv => {
                        self.add_log("Player started in background window".to_string());
                        self.add_log("Continue browsing while video plays".to_string());
                    }
                    PlayMode::MpvInTerminal => {
                        self.add_log("Player started in terminal mode".to_string());
                    }
                }
                // Return to the previous state so user can continue browsing
                self.state = return_state;
            }
        }
    }

    async fn play_stream_detached(&mut self, stream: &Stream) {
        self.add_log(format!("Playing in detached window: {}", stream.name));

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

            // Use disassociated play method for fully independent window
            if let Err(e) = self.player.play_disassociated(&url).await {
                self.state = AppState::Error(format!("Failed to play stream: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Stream started in new independent window".to_string());
                self.add_log("This window won't be affected by other playback".to_string());
            }
        }
    }

    async fn play_stream_in_terminal(&mut self, stream: &Stream) {
        self.add_log(format!("Playing in debug terminal: {}", stream.name));

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
            self.add_log("Launching MPV in terminal for debug output".to_string());

            // Use terminal play method for debugging
            if let Err(e) = self.player.play_in_terminal(&url).await {
                self.state = AppState::Error(format!("Failed to launch terminal: {}", e));
                self.add_log(format!("Terminal launch failed: {}", e));
            } else {
                self.add_log("MPV launched in terminal with verbose output".to_string());
                self.add_log("Check the terminal window for debug information".to_string());
            }
        }
    }

    async fn show_stream_advanced_menu(&mut self, stream: Stream, content_type: ContentType) {
        // Save current state AND items for going back
        self.save_current_navigation_state();
        self.previous_state_before_menu = Some(Box::new(self.state.clone()));
        self.previous_items_before_menu = self.items.clone();
        self.previous_nav_before_menu = NavigationState {
            selected_index: self.selected_index,
            scroll_offset: self.scroll_offset,
            search_query: self.search_query.clone(),
            filtered_indices: self.filtered_indices.clone(),
        };

        // Create menu items
        self.items = vec![
            "Play stream (default .m3u8)".to_string(),
            "Play stream in terminal (.m3u8)".to_string(),
            "Play .ts stream".to_string(),
            "Play .ts stream in terminal".to_string(),
            "Play stream in detached window (.m3u8)".to_string(),
            "Play .ts stream in detached window".to_string(),
            "Back".to_string(),
        ];

        self.selected_index = 0;
        self.filtered_indices = (0..self.items.len()).collect();
        self.search_query.clear();
        self.state = AppState::StreamAdvancedMenu(stream, content_type);
        self.add_log("Advanced menu opened".to_string());
    }

    async fn handle_stream_advanced_menu_selection(
        &mut self,
        stream: Stream,
        _content_type: ContentType,
    ) {
        match self.selected_index {
            0 => {
                // Play stream (default .m3u8) - stay in menu
                self.play_stream(&stream).await;
            }
            1 => {
                // Play stream in terminal (.m3u8) - stay in menu
                self.play_stream_in_terminal(&stream).await;
            }
            2 => {
                // Play .ts stream - stay in menu
                self.play_stream_ts(&stream).await;
            }
            3 => {
                // Play .ts stream in terminal - stay in menu
                self.play_stream_ts_terminal(&stream).await;
            }
            4 => {
                // Play stream in detached window (.m3u8) - stay in menu
                self.play_stream_detached(&stream).await;
            }
            5 => {
                // Play .ts stream in detached window - stay in menu
                self.play_stream_ts_detached(&stream).await;
            }
            6 => {
                // Back - exit menu
                self.restore_previous_state();
            }
            _ => {}
        }
    }

    async fn play_stream_ts(&mut self, stream: &Stream) {
        // Store the current state to return to after starting playback
        let return_state = self.state.clone();

        self.add_log(format!("Playing .ts stream: {}", stream.name));

        if let Some(api) = &self.current_api {
            let url = api.get_stream_url(
                stream.stream_id,
                &stream.stream_type,
                Some("ts"), // Use .ts extension
            );

            self.add_log(format!("URL (.ts): {}", url));

            // Run MPV in TUI-compatible mode (background)
            if let Err(e) = self.player.play_tui(&url).await {
                self.state = AppState::Error(format!("Failed to play stream: {}", e));
                self.add_log(format!("Failed to play stream: {}", e));
            } else {
                self.add_log(format!("Started playing .ts stream: {}", stream.name));
                self.add_log("Player started in background window".to_string());
                // Return to the previous state so user stays in menu
                self.state = return_state;
            }
        }
    }

    async fn play_stream_ts_terminal(&mut self, stream: &Stream) {
        self.add_log(format!("Playing .ts stream in terminal: {}", stream.name));

        if let Some(api) = &self.current_api {
            let url = api.get_stream_url(
                stream.stream_id,
                &stream.stream_type,
                Some("ts"), // Use .ts extension
            );

            self.add_log(format!("URL (.ts): {}", url));

            // Use terminal play method for debugging
            if let Err(e) = self.player.play_in_terminal(&url).await {
                self.state = AppState::Error(format!("Failed to launch terminal: {}", e));
                self.add_log(format!("Terminal launch failed: {}", e));
            } else {
                self.add_log("MPV launched in terminal with verbose output".to_string());
                self.add_log("Check the terminal window for debug information".to_string());
            }
        }
    }

    async fn play_stream_ts_detached(&mut self, stream: &Stream) {
        self.add_log(format!(
            "Playing .ts stream in detached window: {}",
            stream.name
        ));

        if let Some(api) = &self.current_api {
            let url = api.get_stream_url(
                stream.stream_id,
                &stream.stream_type,
                Some("ts"), // Use .ts extension
            );

            self.add_log(format!("URL (.ts): {}", url));

            // Use detached play method
            match self.player.play_detached(&url).await {
                Ok(_) => {
                    self.add_log(format!("Detached player started for: {}", stream.name));
                    self.add_log("Player running in separate window".to_string());
                }
                Err(e) => {
                    self.state = AppState::Error(format!("Failed to play stream: {}", e));
                    self.add_log(format!("Failed to play stream: {}", e));
                }
            }
        }
    }

    fn restore_previous_state(&mut self) {
        // Restore to the previous state before the advanced menu
        if let Some(previous_state) = self.previous_state_before_menu.take() {
            // Restore the state
            self.state = *previous_state;

            // Restore the items list
            self.items = self.previous_items_before_menu.clone();

            // Restore the navigation state (selected index, scroll, filter)
            self.selected_index = self.previous_nav_before_menu.selected_index;
            self.scroll_offset = self.previous_nav_before_menu.scroll_offset;
            self.search_query = self.previous_nav_before_menu.search_query.clone();
            self.filtered_indices = self.previous_nav_before_menu.filtered_indices.clone();

            // Clear the saved state
            self.previous_items_before_menu.clear();
            self.previous_nav_before_menu = NavigationState::new();
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

            // Use play mode from configuration
            let result = match self.config.settings.play_mode {
                PlayMode::Mpv => self.player.play_tui(&url).await,
                PlayMode::MpvInTerminal => self.player.play_in_terminal(&url).await,
            };

            if let Err(e) = result {
                self.state = AppState::Error(format!("Failed to play episode: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                match self.config.settings.play_mode {
                    PlayMode::Mpv => {
                        self.add_log("Player started in background window".to_string());
                        self.add_log("Continue browsing while video plays".to_string());
                    }
                    PlayMode::MpvInTerminal => {
                        self.add_log("Player started in terminal mode".to_string());
                    }
                }
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
                items.push("  > Play in Detached Window".to_string());
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

            // Use play mode from configuration
            let result = match self.config.settings.play_mode {
                PlayMode::Mpv => self.player.play_tui(&url).await,
                PlayMode::MpvInTerminal => self.player.play_in_terminal(&url).await,
            };

            if let Err(e) = result {
                self.state = AppState::Error(format!("Failed to play movie: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                match self.config.settings.play_mode {
                    PlayMode::Mpv => {
                        self.add_log("Player started in background window".to_string());
                        self.add_log("Continue browsing while video plays".to_string());
                    }
                    PlayMode::MpvInTerminal => {
                        self.add_log("Player started in terminal mode".to_string());
                    }
                }
                // Return to the VOD info state so user can see the info
                self.state = return_state;
            }
        }
    }

    async fn play_vod_stream_detached(&mut self, stream: &Stream) {
        self.add_log(format!("Playing in detached window: {}", stream.name));

        if let Some(api) = &self.current_api {
            // Use the container extension from VOD info if available
            let extension = self
                .vod_info
                .as_ref()
                .map(|info| info.movie_data.container_extension.as_str());

            let url = api.get_stream_url(stream.stream_id, "movie", extension);

            // Log the stream URL
            self.add_log(format!("Stream URL: {}", url));

            // Use disassociated play method for fully independent window
            if let Err(e) = self.player.play_disassociated(&url).await {
                self.state = AppState::Error(format!("Failed to play movie: {}", e));
                self.add_log(format!("Playback failed: {}", e));
            } else {
                self.add_log("Movie started in new independent window".to_string());
                self.add_log("This window won't be affected by other playback".to_string());
            }
        }
    }

    fn save_current_navigation_state(&mut self) {
        let nav_state = NavigationState {
            selected_index: self.selected_index,
            scroll_offset: self.scroll_offset,
            search_query: self.search_query.clone(),
            filtered_indices: self.filtered_indices.clone(),
        };

        match self.state.clone() {
            AppState::ProviderSelection => {
                self.provider_selection_state = nav_state;
            }
            AppState::MainMenu => {
                self.main_menu_state = nav_state;
            }
            AppState::CategorySelection(content_type) => {
                self.category_selection_states
                    .insert(content_type, nav_state);
            }
            AppState::StreamSelection(content_type, category) => {
                self.stream_selection_states
                    .insert((content_type, category.category_id.clone()), nav_state);
            }
            AppState::SeasonSelection(_) => {
                self.season_selection_state = nav_state;
            }
            AppState::CrossProviderFavourites => {
                self.cross_provider_favourites_state = nav_state;
            }
            AppState::Configuration => {
                self.config_state = nav_state;
            }
            _ => {}
        }
    }

    fn restore_navigation_state(&mut self, for_state: &AppState) {
        let nav_state = match for_state {
            AppState::ProviderSelection => self.provider_selection_state.clone(),
            AppState::MainMenu => self.main_menu_state.clone(),
            AppState::CategorySelection(content_type) => self
                .category_selection_states
                .get(content_type)
                .cloned()
                .unwrap_or_else(NavigationState::new),
            AppState::StreamSelection(content_type, category) => self
                .stream_selection_states
                .get(&(*content_type, category.category_id.clone()))
                .cloned()
                .unwrap_or_else(NavigationState::new),
            AppState::SeasonSelection(_) => self.season_selection_state.clone(),
            AppState::CrossProviderFavourites => self.cross_provider_favourites_state.clone(),
            AppState::Configuration => self.config_state.clone(),
            _ => NavigationState::new(),
        };

        // Ensure the restored index is within bounds for current items
        self.selected_index = nav_state
            .selected_index
            .min(self.items.len().saturating_sub(1));
        self.scroll_offset = nav_state.scroll_offset;

        // If the saved state has empty filtered_indices and no search query,
        // initialize it to show all items
        if nav_state.filtered_indices.is_empty() && nav_state.search_query.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
            self.search_query = nav_state.search_query;
        } else {
            // Validate that filtered indices are within bounds
            self.filtered_indices = nav_state
                .filtered_indices
                .into_iter()
                .filter(|&idx| idx < self.items.len())
                .collect();
            self.search_query = nav_state.search_query;

            // If all filtered indices were out of bounds, reset to show all
            if self.filtered_indices.is_empty() && self.search_query.is_empty() {
                self.filtered_indices = (0..self.items.len()).collect();
            }
        }

        // Ensure selected index is in the filtered list
        if !self.filtered_indices.is_empty()
            && !self.filtered_indices.contains(&self.selected_index)
        {
            self.selected_index = self.filtered_indices[0];
        }
    }

    fn stop_playing(&mut self) {
        // Stop the player process
        let player = self.player.clone();
        tokio::spawn(async move {
            let _ = player.stop_tui().await;
        });

        self.state = AppState::MainMenu;
        // Restore main menu navigation state
        self.restore_navigation_state(&AppState::MainMenu);
        self.update_main_menu_items();
        self.add_log("Stopped playback".to_string());
    }

    async fn refresh_cache(&mut self) -> Option<Action> {
        // Return a special action to exit TUI and run cache refresh
        Some(Action::CacheRefresh)
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

    fn get_current_category(&self) -> Option<Category> {
        // selected_index is already the actual index in the categories array
        if self.selected_index < self.categories.len() {
            Some(self.categories[self.selected_index].clone())
        } else {
            None
        }
    }

    /// Clear internal TUI caches
    pub fn clear_internal_caches(&mut self) {
        self.cached_categories.clear();
        self.cached_streams.clear();
    }

    fn add_log(&mut self, message: String) {
        self.logs.push((Local::now(), message));
        // Keep only last 100 logs
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }

    fn ensure_selected_visible(&mut self) {
        // Make sure the selected item is visible on screen
        let visible_height = self.visible_height.max(1);

        // Get the actual position in the filtered list
        if self.filtered_indices.is_empty() {
            return; // Nothing to show, nothing to scroll
        }

        let position = self
            .filtered_indices
            .iter()
            .position(|&i| i == self.selected_index)
            .unwrap_or(0);

        // Keep 1 line of context when scrolling (if possible)
        let context_lines = 1;

        // If selected item is above visible area, scroll up
        if position < self.scroll_offset + context_lines {
            self.scroll_offset = position.saturating_sub(context_lines);
        }
        // If selected item is below visible area, scroll down
        else if position >= self.scroll_offset + visible_height - context_lines {
            let max_scroll = self.filtered_indices.len().saturating_sub(visible_height);
            self.scroll_offset = (position + context_lines + 1)
                .saturating_sub(visible_height)
                .min(max_scroll);
        }
    }

    fn reset_filter(&mut self) {
        self.search_query.clear();
        self.search_active = false;
        self.filtered_indices = (0..self.items.len()).collect();
        self.status_message = None;
    }
}
