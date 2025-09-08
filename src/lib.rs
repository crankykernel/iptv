// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

pub mod config;
pub mod menu;
pub mod player;
pub mod xtream_api;

pub use config::Config;
pub use menu::MenuSystem;
pub use player::Player;
pub use xtream_api::XTreamAPI;
