// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod config;
pub mod xtream_api;
pub mod menu;
pub mod player;

pub use config::Config;
pub use xtream_api::XTreamAPI;
pub use menu::MenuSystem;
pub use player::Player;