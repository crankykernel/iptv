// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use iptv::{Config, MenuSystem, Player};

#[derive(Parser)]
#[command(name = "iptv")]
#[command(about = "A terminal-based IPTV player with XTream API support")]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine config file path
    let config_path = cli.config.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .join("config.toml")
    });

    // Load configuration
    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        eprintln!("Config file not found at: {}", config_path.display());
        eprintln!("Creating example config at: config.example.toml");
        eprintln!("Please copy and edit it to config.toml");

        let example_config = Config::default();
        example_config.save("config.example.toml")?;

        return Ok(());
    };

    // Initialize player
    let player = Player::new(config.player.clone());

    // Initialize and run menu system
    let mut menu_system = MenuSystem::new(config.providers, player, config.ui.page_size);
    menu_system.run().await?;

    Ok(())
}
