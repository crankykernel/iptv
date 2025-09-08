// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use iptv::{Config, MenuSystem, Player};

#[derive(Parser)]
#[command(name = "iptv")]
#[command(about = "A terminal-based IPTV player with XTream API support")]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable verbose (debug) logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(false)
        .without_time()
        .init();

    // Determine config file path
    let config_path = cli.config.unwrap_or_else(|| {
        // First check for config.toml in current directory
        let local_config = std::env::current_dir()
            .unwrap_or_default()
            .join("config.toml");

        if local_config.exists() {
            return local_config;
        }

        // Fallback to ~/.config/iptv.toml
        if let Some(home_dir) = std::env::var_os("HOME") {
            let user_config = PathBuf::from(home_dir).join(".config").join("iptv.toml");

            if user_config.exists() {
                return user_config;
            }
        }

        // Default to current directory if neither exists
        local_config
    });

    // Load configuration
    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        eprintln!("Config file not found at: {}", config_path.display());
        eprintln!("Also checked: ~/.config/iptv.toml");
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
