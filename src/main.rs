// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use clap::{Parser, Subcommand};
use inquire::{Password, Text};
use std::fs::File;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

use iptv::config::ProviderConfig;
use iptv::xtream_api::{FavouriteStream, XTreamAPI};
use iptv::{Config, MenuSystem, Player};
use std::process::{Command, Stdio};

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

    /// Use TUI (Terminal User Interface) mode
    #[arg(long)]
    tui: bool,

    /// Enable debug logging to file (iptv_debug.log)
    #[arg(long)]
    debug_log: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch rofi menu with favourites
    Rofi,
    /// Interactively add a new Xtreme API provider to the configuration
    AddProvider,
}

async fn run_rofi_menu(providers: Vec<ProviderConfig>, player: Player) -> Result<()> {
    if providers.is_empty() {
        eprintln!("No providers configured. Please check your config file.");
        return Ok(());
    }

    // Check if rofi is available - just try to run it, don't worry about warnings
    if !Command::new("rofi")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        eprintln!("Error: 'rofi' command not found or not working. Please install rofi.");
        return Ok(());
    }

    // Collect favourites from all providers with provider info
    #[derive(Clone)]
    struct FavouriteWithProvider {
        favourite: FavouriteStream,
        provider_name: Option<String>,
        provider_config: ProviderConfig,
    }

    let mut all_favourites = Vec::new();

    println!("Loading favourites from {} provider(s)...", providers.len());

    for provider in &providers {
        println!(
            "Connecting to provider: {}",
            provider.name.as_ref().unwrap_or(&provider.url)
        );

        let api = XTreamAPI::new(
            provider.url.clone(),
            provider.username.clone(),
            provider.password.clone(),
            provider.name.clone(),
        )?;

        // Get favourites from this provider using the new favourites manager
        let provider_favourites = match api.favourites_manager.get_favourites(&api.provider_hash) {
            Ok(favs) => {
                if !favs.is_empty() {
                    println!(
                        "Loaded {} favourites from {}",
                        favs.len(),
                        provider.name.as_ref().unwrap_or(&provider.url)
                    );
                }
                favs
            }
            Err(e) => {
                println!(
                    "Error loading favourites from {}: {}",
                    provider.name.as_ref().unwrap_or(&provider.url),
                    e
                );
                Vec::new()
            }
        };

        // Store favourites with their provider info
        for favourite in provider_favourites {
            all_favourites.push(FavouriteWithProvider {
                favourite,
                provider_name: provider.name.clone(),
                provider_config: provider.clone(),
            });
        }
    }

    let favourites = all_favourites;

    if favourites.is_empty() {
        println!("No favourites found. Use the interactive menu to add favourites first.");
        return Ok(());
    }

    // Prepare rofi input: format favourites for display with provider names
    let mut rofi_input = String::new();
    for fav_with_provider in &favourites {
        // Include provider name for clarity when multiple providers have favourites
        let provider_name = fav_with_provider
            .provider_name
            .as_ref()
            .map(|name| format!(" [{}]", name))
            .unwrap_or_default();
        rofi_input.push_str(&format!(
            "{}{}\n",
            fav_with_provider.favourite.name, provider_name
        ));
    }

    // Launch rofi to select a favourite
    let mut rofi_cmd = Command::new("rofi");
    rofi_cmd
        .arg("-dmenu")
        .arg("-hover-select")
        .arg("-me-select-entry")
        .arg("")
        .arg("-me-accept-entry")
        .arg("MousePrimary")
        .arg("-i") // case insensitive
        .arg("-p")
        .arg("Select favourite stream:")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut rofi_process = rofi_cmd.spawn()?;

    // Write favourites to rofi's stdin
    if let Some(stdin) = rofi_process.stdin.as_mut() {
        use std::io::Write;
        stdin.write_all(rofi_input.as_bytes())?;
    }

    let output = rofi_process.wait_with_output()?;

    if !output.status.success() {
        // Check if there's stderr output to help debug
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            eprintln!("rofi error: {}", stderr.trim());
        } else {
            println!("User cancelled selection or rofi exited");
        }
        return Ok(());
    }

    let selected_display = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Find the selected favourite by matching the display name
    let mut selected_item = None;
    for fav_with_provider in &favourites {
        let provider_name = fav_with_provider
            .provider_name
            .as_ref()
            .map(|name| format!(" [{}]", name))
            .unwrap_or_default();
        let display_name = format!("{}{}", fav_with_provider.favourite.name, provider_name);

        if display_name == selected_display {
            selected_item = Some(fav_with_provider);
            break;
        }
    }

    if let Some(selected_item) = selected_item {
        // Create API for the selected provider to get stream URL
        let api = XTreamAPI::new(
            selected_item.provider_config.url.clone(),
            selected_item.provider_config.username.clone(),
            selected_item.provider_config.password.clone(),
            selected_item.provider_config.name.clone(),
        )?;

        // Get the stream URL and start playing in background
        let stream_url = api.get_stream_url(
            selected_item.favourite.stream_id,
            &selected_item.favourite.stream_type,
            None,
        );
        println!("Starting: {}", selected_item.favourite.name);

        // Start mpv in background and exit immediately
        // Use blocking to run the async play method
        tokio::runtime::Runtime::new()?.block_on(async { player.play(&stream_url).await })?;
        println!("Player started in background");
    } else {
        eprintln!("Selected favourite not found: {}", selected_display);
    }

    Ok(())
}

async fn add_provider_interactively(config_path: PathBuf) -> Result<()> {
    println!("Adding a new Xtreme API provider to your configuration");
    println!("Please provide the following information:");
    println!();

    // Prompt for provider name
    let name = Text::new("Provider name (user-friendly identifier):")
        .with_help_message("This is just for your reference, e.g. 'My IPTV Service'")
        .prompt()?;

    // Validate name is not empty
    if name.trim().is_empty() {
        anyhow::bail!("Provider name cannot be empty");
    }

    // Prompt for URL
    let url = Text::new("Xtreme API URL:")
        .with_help_message("e.g. https://your-server.com:port/player_api.php")
        .prompt()?;

    // Validate URL is not empty and looks like a URL
    if url.trim().is_empty() {
        anyhow::bail!("URL cannot be empty");
    }

    // Basic URL validation
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("URL must start with http:// or https://");
    }

    // Prompt for username
    let username = Text::new("Username:").prompt()?;

    // Validate username is not empty
    if username.trim().is_empty() {
        anyhow::bail!("Username cannot be empty");
    }

    // Prompt for password
    let password = Password::new("Password:").without_confirmation().prompt()?;

    // Validate password is not empty
    if password.trim().is_empty() {
        anyhow::bail!("Password cannot be empty");
    }

    // Create new provider config
    let new_provider = ProviderConfig {
        name: Some(name.trim().to_string()),
        url: url.trim().to_string(),
        username: username.trim().to_string(),
        password: password.trim().to_string(),
    };

    // Load existing config or create new one
    let mut config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        // Ensure config directory exists
        let _ = Config::ensure_config_dir();
        Config::default()
    };

    // Add the new provider
    config.providers.push(new_provider);

    // Save the updated config
    config.save(&config_path)?;

    println!();
    println!("✓ Provider '{}' added successfully!", name.trim());
    println!("Configuration saved to: {}", config_path.display());
    println!();
    println!("You can now use this provider in the IPTV application.");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose || cli.debug_log {
        EnvFilter::new("debug,reqwest=warn,h2=warn,hyper=warn")
    } else {
        EnvFilter::new("info,reqwest=warn,h2=warn,hyper=warn")
    };

    if cli.debug_log && cli.tui {
        // Setup file logging for TUI debug mode
        let debug_file = File::create("iptv_debug.log")?;

        // In TUI mode with debug, only log to file
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(debug_file)
            .with_ansi(false)
            .with_target(true)
            .with_line_number(true)
            .init();
    } else {
        // Normal logging to console
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_level(true)
            .without_time()
            .init();
    }

    // Determine config file path
    let config_path = cli.config.unwrap_or_else(|| {
        // First check for config.toml in current directory
        let local_config = std::env::current_dir()
            .unwrap_or_default()
            .join("config.toml");

        if local_config.exists() {
            return local_config;
        }

        // Check for ~/.config/iptv/config.toml
        if let Some(config_path) = Config::default_config_path()
            && config_path.exists()
        {
            return config_path;
        }

        // Default to new location
        Config::default_config_path().unwrap_or(local_config)
    });

    // Load configuration
    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        // Ensure config directory exists for new location
        let _ = Config::ensure_config_dir();

        eprintln!("Config file not found at: {}", config_path.display());
        eprintln!("Expected locations:");
        eprintln!("  1. ./config.toml (current directory)");
        eprintln!("  2. ~/.config/iptv/config.toml (recommended)");
        eprintln!();
        eprintln!("Creating example config at: config.example.toml");
        eprintln!("Please copy and edit it to one of the locations above");

        let example_config = Config::default();
        example_config.save("config.example.toml")?;

        return Ok(());
    };

    // Initialize player (automatically uses MPV if available)
    let player = Player::new();

    // Handle subcommands
    match cli.command {
        Some(Commands::Rofi) => {
            run_rofi_menu(config.providers, player).await?;
        }
        Some(Commands::AddProvider) => {
            add_provider_interactively(config_path).await?;
        }
        None => {
            // Check if TUI mode is requested
            if cli.tui {
                // Run TUI mode
                iptv::tui::run_tui(config.providers, player).await?;
            } else {
                // Initialize and run menu system
                let mut menu_system =
                    MenuSystem::new(config.providers, player, config.ui.page_size);
                menu_system.run().await?;
            }
        }
    }

    Ok(())
}
