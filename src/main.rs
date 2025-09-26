// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use iptv::config::ProviderConfig;
use iptv::xtream::XTreamAPI;
use iptv::{Config, Player};

mod cli;
use cli::{CacheCommand, CommandContext, ContentType, OutputFormat, SearchCommand};

fn cargo_style() -> Styles {
    Styles::styled()
        .header(AnsiColor::Green.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default())
}

#[derive(Parser)]
#[command(name = "iptv")]
#[command(about = "A terminal-based IPTV player with XTream API support")]
#[command(version)]
#[command(styles = cargo_style())]
struct Cli {
    /// Enable verbose (debug) logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable debug logging to file (iptv_debug.log)
    #[arg(long, global = true)]
    debug_log: bool,

    /// Provider name to open directly (case-insensitive, for TUI mode)
    #[arg(short, long, global = true)]
    provider: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch interactive TUI (default if no command given)
    Tui {
        /// Provider name to open directly (case-insensitive)
        #[arg(short, long)]
        provider: Option<String>,
    },

    /// Launch rofi menu with favourites
    Rofi,

    /// Command-line interface for scriptable operations
    Cli(CliCommands),

    /// Execute raw API calls
    Api(ApiCommands),
}

#[derive(Parser)]
#[command(styles = cargo_style())]
struct CliCommands {
    /// Provider name to use (or set IPTV_PROVIDER env var)
    #[arg(short, long)]
    provider: Option<String>,

    #[command(subcommand)]
    command: CliSubcommands,
}

#[derive(Subcommand)]
enum CliSubcommands {
    /// Search content across providers
    Search {
        /// Search query
        query: String,
        /// Content type to search (live, movie, series)
        #[arg(short = 't', long)]
        r#type: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Manage cache
    #[command(subcommand)]
    Cache(CacheSubCommand),
}

#[derive(Subcommand)]
enum CacheSubCommand {
    /// Refresh cache
    Refresh,
    /// Clear cache
    Clear,
}

#[derive(Parser)]
#[command(styles = cargo_style())]
struct ApiCommands {
    /// Provider name to use (case-insensitive)
    #[arg(short, long)]
    provider: Option<String>,

    #[command(subcommand)]
    command: ApiSubcommand,
}

#[derive(Subcommand)]
enum ApiSubcommand {
    /// Get user info
    UserInfo,
    /// Get live categories
    LiveCategories,
    /// Get VOD categories
    VodCategories,
    /// Get series categories
    SeriesCategories,
    /// Get live streams
    LiveStreams {
        #[arg(short, long)]
        category: Option<String>,
    },
    /// Get VOD streams
    VodStreams {
        #[arg(short, long)]
        category: Option<String>,
    },
    /// Get series
    Series {
        #[arg(short, long)]
        category: Option<String>,
    },
    /// Get series info
    SeriesInfo { id: u32 },
    /// Get VOD info
    VodInfo { id: u32 },
}

async fn run_rofi_menu(providers: Vec<ProviderConfig>, player: Player) -> Result<()> {
    use iptv::favourites::FavouritesManager;
    use iptv::xtream::FavouriteStream;
    use std::io::Write;

    if providers.is_empty() {
        eprintln!("No providers configured. Please check your config file.");
        return Ok(());
    }

    // Check if rofi is available
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

    tracing::debug!("Loading favourites from {} provider(s)...", providers.len());

    for provider in &providers {
        tracing::debug!(
            "Connecting to provider: {}",
            provider.name.as_ref().unwrap_or(&provider.url)
        );

        let api = XTreamAPI::new_with_id(
            provider.url.clone(),
            provider.username.clone(),
            provider.password.clone(),
            provider.name.clone(),
            provider.id.clone(),
        )?;

        // Get favourites from this provider using the provider hash from the API
        let favourites_manager = FavouritesManager::new()?;

        let provider_favourites =
            match tokio::time::timeout(std::time::Duration::from_secs(5), async {
                favourites_manager.get_favourites(&api.provider_hash)
            })
            .await
            {
                Ok(Ok(favs)) => {
                    if !favs.is_empty() {
                        tracing::debug!(
                            "Loaded {} favourites from {}",
                            favs.len(),
                            provider.name.as_ref().unwrap_or(&provider.url)
                        );
                    }
                    favs
                }
                Ok(Err(e)) => {
                    tracing::warn!(
                        "Error loading favourites from {}: {}",
                        provider.name.as_ref().unwrap_or(&provider.url),
                        e
                    );
                    Vec::new()
                }
                Err(_) => {
                    tracing::warn!(
                        "Timeout loading favourites from {}",
                        provider.name.as_ref().unwrap_or(&provider.url)
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
        println!("No favourites found. Use the TUI to add favourites first.");
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
        .arg("IPTV Favourites")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());

    let mut rofi_process = rofi_cmd.spawn()?;

    // Send the favourites to rofi's stdin
    if let Some(stdin) = rofi_process.stdin.as_mut() {
        stdin.write_all(rofi_input.as_bytes())?;
    }

    // Get the selection from rofi
    let output = rofi_process.wait_with_output()?;

    if !output.status.success() {
        // User cancelled or error
        return Ok(());
    }

    let selected = String::from_utf8_lossy(&output.stdout);
    let selected = selected.trim();

    if selected.is_empty() {
        return Ok(());
    }

    // Find the selected favourite (match the display name with provider suffix)
    let selected_fav = favourites.iter().find(|fav_with_provider| {
        let provider_name = fav_with_provider
            .provider_name
            .as_ref()
            .map(|name| format!(" [{}]", name))
            .unwrap_or_default();
        let display_name = format!("{}{}", fav_with_provider.favourite.name, provider_name);
        display_name == selected
    });

    if let Some(fav_with_provider) = selected_fav {
        // Create API for the selected provider
        let api = XTreamAPI::new_with_id(
            fav_with_provider.provider_config.url.clone(),
            fav_with_provider.provider_config.username.clone(),
            fav_with_provider.provider_config.password.clone(),
            fav_with_provider.provider_config.name.clone(),
            fav_with_provider.provider_config.id.clone(),
        )?;

        // Get the stream URL based on stream type
        let url = match fav_with_provider.favourite.stream_type.as_str() {
            "live" => api.get_stream_url(fav_with_provider.favourite.stream_id, "live", None),
            "movie" => api.get_stream_url(fav_with_provider.favourite.stream_id, "movie", None),
            "series" => {
                // For series, we'd need to handle episode selection, but for simplicity,
                // we'll just show an error
                eprintln!("Series playback not supported in rofi mode. Use TUI mode instead.");
                return Ok(());
            }
            _ => {
                eprintln!(
                    "Unknown stream type: {}",
                    fav_with_provider.favourite.stream_type
                );
                return Ok(());
            }
        };

        tracing::info!("Starting playback of: {}", fav_with_provider.favourite.name);

        // Play in detached mode so rofi can exit cleanly
        player.play_detached(&url).await?;
    }

    Ok(())
}

async fn run_api_command(_provider: &str, api: &mut XTreamAPI, cmd: ApiSubcommand) -> Result<()> {
    // Return raw JSON responses without any interpretation or deserialization
    let result = match cmd {
        ApiSubcommand::UserInfo => api.make_request_raw("get_user_info", None).await?,
        ApiSubcommand::LiveCategories => api.make_request_raw("get_live_categories", None).await?,
        ApiSubcommand::VodCategories => api.make_request_raw("get_vod_categories", None).await?,
        ApiSubcommand::SeriesCategories => {
            api.make_request_raw("get_series_categories", None).await?
        }
        ApiSubcommand::LiveStreams { category } => {
            api.make_request_raw("get_live_streams", category.as_deref())
                .await?
        }
        ApiSubcommand::VodStreams { category } => {
            api.make_request_raw("get_vod_streams", category.as_deref())
                .await?
        }
        ApiSubcommand::Series { category } => {
            api.make_request_raw("get_series", category.as_deref())
                .await?
        }
        ApiSubcommand::SeriesInfo { id } => {
            api.make_info_request_raw("get_series_info", id).await?
        }
        ApiSubcommand::VodInfo { id } => api.make_info_request_raw("get_vod_info", id).await?,
    };

    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging
    if cli.debug_log {
        let file = File::create("iptv_debug.log")?;
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(file)
            .with_ansi(false)
            .with_level(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true);

        tracing_subscriber::registry()
            .with(file_layer)
            .with(
                EnvFilter::from_default_env()
                    .add_directive("iptv=debug".parse()?)
                    .add_directive("hyper_util=error".parse()?),
            )
            .init();
    } else if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env()
                    .add_directive(tracing::Level::DEBUG.into())
                    .add_directive("hyper_util=error".parse()?),
            )
            .init();
    } else if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env().add_directive("hyper_util=error".parse()?),
            )
            .init();
    }

    // Load configuration
    let config_path = dirs::config_dir()
        .map(|p| p.join("iptv").join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    let mut config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        Config::default()
    };

    // Check if we should run the interactive setup
    if iptv::setup::should_run_setup(&config_path, &config) {
        // Only run setup for TUI mode or no command (which defaults to TUI)
        match &cli.command {
            Some(Commands::Tui { .. }) | None => {
                iptv::setup::interactive_provider_setup().await?;
                // Reload the config after setup
                config = Config::load(&config_path)?;
            }
            _ => {
                // For other commands, just warn about missing providers
                if config.providers.is_empty() {
                    eprintln!(
                        "No providers configured. Please run 'iptv --tui' to set up a provider."
                    );
                    return Ok(());
                }
            }
        }
    }

    // Create player
    let player = Player::new();

    // Execute command
    match cli.command {
        Some(Commands::Tui { provider }) => {
            // Launch TUI with provider from subcommand or global option
            let provider_to_use = provider.or(cli.provider.clone());
            iptv::run_tui(config, player, provider_to_use).await?;
        }
        None => {
            // No command given, launch TUI with global provider option if specified
            iptv::run_tui(config, player, cli.provider.clone()).await?;
        }

        Some(Commands::Cli(cli_args)) => {
            // Get provider from cli args or env var
            let selected_provider = cli_args
                .provider
                .or_else(|| std::env::var("IPTV_PROVIDER").ok());

            // Create command context
            let context = CommandContext::new(config.providers.clone(), selected_provider, false);

            match cli_args.command {
                CliSubcommands::Search {
                    query,
                    r#type,
                    format,
                } => {
                    let content_type = r#type.map(|t| ContentType::from_str(&t)).transpose()?;
                    let output_format = OutputFormat::from_str(&format)?;
                    let cmd = SearchCommand {
                        query,
                        content_type,
                        format: output_format,
                    };
                    cmd.execute(context).await?;
                }

                CliSubcommands::Cache(cache_cmd) => {
                    let cmd = match cache_cmd {
                        CacheSubCommand::Refresh => CacheCommand::Refresh,
                        CacheSubCommand::Clear => CacheCommand::Clear,
                    };
                    cmd.execute(context).await?;
                }
            }
        }

        Some(Commands::Rofi) => {
            run_rofi_menu(config.providers, player).await?;
        }

        Some(Commands::Api(api_cmds)) => {
            // Use provider from command line option only
            let selected_provider = api_cmds.provider;

            // Create command context with case-insensitive provider selection
            let context = CommandContext::new(config.providers.clone(), selected_provider, false);

            let (mut api, provider_name) = context.get_single_provider().await?;
            eprintln!("Using provider: {}", provider_name);
            run_api_command(&provider_name, &mut api, api_cmds.command).await?;
        }
    }

    Ok(())
}
