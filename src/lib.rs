// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod cache;
pub mod config;
pub mod favourites;
pub mod ignore;
pub mod player;
pub mod tui;
pub mod xtream;

pub use cache::CacheManager;
pub use config::Config;
pub use favourites::FavouritesManager;
pub use player::Player;
pub use xtream::XTreamAPI;

use anyhow::Result;

/// Run the TUI application
pub async fn run_tui(config: Config, player: Player) -> Result<()> {
    tui::run_tui(config.providers, player).await
}
