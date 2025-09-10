// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod cache;
pub mod cli;
pub mod config;
pub mod favourites;
pub mod mpv_player;
pub mod player;
pub mod tui;
pub mod xtream_api;

pub use cache::CacheManager;
pub use cli::MenuSystem;
pub use config::Config;
pub use favourites::FavouritesManager;
pub use player::Player;
pub use xtream_api::XTreamAPI;
