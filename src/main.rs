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
use cli::{
    CacheCommand, CommandContext, ContentType, FavoritesCommand, InfoCommand, ListCommand,
    OutputFormat, PlayCommand, ProvidersCommand, SearchCommand,
};

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

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch interactive TUI (default if no command given)
    Tui,

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
    /// Play stream/movie/episode by ID
    Play {
        /// Stream/Movie/Series ID
        id: u32,
        /// Content type (live, movie, series)
        #[arg(short = 't', long)]
        r#type: Option<String>,
        /// Play in detached window
        #[arg(short, long)]
        detached: bool,
    },

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

    /// List streams/movies/series
    #[command(subcommand)]
    List(ListSubCommand),

    /// Get detailed information about content
    Info {
        /// Content ID
        id: u32,
        /// Content type (live, movie, series)
        #[arg(short = 't', long)]
        r#type: String,
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Get stream URL
    Url {
        /// Content ID
        id: u32,
        /// Content type (live, movie, series)
        #[arg(short = 't', long)]
        r#type: String,
    },

    /// Manage favorites
    #[command(subcommand)]
    Fav(FavCommand),

    /// Manage cache
    #[command(subcommand)]
    Cache(CacheSubCommand),

    /// Manage providers
    #[command(subcommand)]
    Providers(ProvidersSubCommand),

    /// Interactively add a new provider
    AddProvider,
}

#[derive(Subcommand)]
enum ListSubCommand {
    /// List live TV streams
    Live {
        /// Category ID for filtering
        #[arg(long)]
        category: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// List movies/VOD
    Movie {
        /// Category ID for filtering
        #[arg(long)]
        category: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// List movies/VOD (alias for movie)
    Movies {
        /// Category ID for filtering
        #[arg(long)]
        category: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// List movies/VOD (alias for movie)
    Vod {
        /// Category ID for filtering
        #[arg(long)]
        category: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// List TV series
    Series {
        /// Category ID for filtering
        #[arg(long)]
        category: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
    /// List TV series (alias for series)
    Tv {
        /// Category ID for filtering
        #[arg(long)]
        category: Option<String>,
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,
    },
}

#[derive(Subcommand)]
enum FavCommand {
    /// List favorites
    List {
        /// Output format (text, json, m3u)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Add to favorites
    Add {
        /// Content ID
        id: u32,
        /// Content type (live, movie, series)
        #[arg(short = 't', long)]
        r#type: String,
        /// Optional name override
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Remove from favorites
    Remove {
        /// Content ID
        id: u32,
    },
}

#[derive(Subcommand)]
enum CacheSubCommand {
    /// Refresh cache
    Refresh,
    /// Clear cache
    Clear,
}

#[derive(Subcommand)]
enum ProvidersSubCommand {
    /// List configured providers
    List {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Test provider connections
    Test {
        /// Optional provider name to test
        name: Option<String>,
    },
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

async fn add_provider_interactively(config_path: PathBuf) -> Result<()> {
    use std::io::{self, Write};

    println!("Adding a new Xtreme API provider to your configuration");
    println!("Please provide the following information:");

    print!("Provider Name (e.g., 'MyIPTV'): ");
    io::stdout().flush()?;
    let mut name = String::new();
    io::stdin().read_line(&mut name)?;
    let name = name.trim().to_string();

    print!("Server URL (e.g., http://example.com:8080): ");
    io::stdout().flush()?;
    let mut url = String::new();
    io::stdin().read_line(&mut url)?;
    let url = url.trim().to_string();

    print!("Username: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    print!("Password: ");
    io::stdout().flush()?;
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;
    let password = password.trim().to_string();

    // Test the connection
    println!("\nTesting connection...");
    let mut test_api = XTreamAPI::new(
        url.clone(),
        username.clone(),
        password.clone(),
        Some(name.clone()),
    )?;

    match test_api.get_user_info().await {
        Ok(info) => {
            println!("✓ Connection successful!");
            println!("  Account: {}", info.username);
            println!("  Status: {}", info.status);
        }
        Err(e) => {
            eprintln!("✗ Connection failed: {}", e);
            eprintln!("Provider will be added anyway, but please check your credentials.");
        }
    }

    // Load existing config or create new
    let mut config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        Config::default()
    };

    // Add the new provider
    let provider = ProviderConfig {
        id: None,
        name: Some(name.clone()),
        url,
        username,
        password,
    };

    config.providers.push(provider);

    // Save the updated config
    config.save(&config_path)?;

    println!(
        "\n✓ Provider '{}' has been added to {}",
        name,
        config_path.display()
    );

    Ok(())
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

    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        eprintln!("No configuration file found at: {}", config_path.display());
        eprintln!("Run 'iptv cli add-provider' to create one.");
        Config::default()
    };

    // Create player
    let player = Player::new();

    // Execute command
    match cli.command {
        Some(Commands::Tui) | None => {
            // Launch TUI
            if config.providers.is_empty() {
                eprintln!("No providers configured. Run 'iptv cli add-provider' to add one.");
                return Ok(());
            }
            iptv::run_tui(config, player).await?;
        }

        Some(Commands::Cli(cli_args)) => {
            // Get provider from cli args or env var
            let selected_provider = cli_args
                .provider
                .or_else(|| std::env::var("IPTV_PROVIDER").ok());

            // Create command context
            let context = CommandContext::new(config.providers.clone(), selected_provider, false);

            match cli_args.command {
                CliSubcommands::Play {
                    id,
                    r#type,
                    detached,
                } => {
                    let content_type = r#type.map(|t| ContentType::from_str(&t)).transpose()?;
                    let cmd = PlayCommand {
                        id,
                        content_type,
                        detached,
                    };
                    cmd.execute(context, player).await?;
                }

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

                CliSubcommands::List(list_cmd) => {
                    let (content_type, category, format, limit) = match list_cmd {
                        ListSubCommand::Live {
                            category,
                            format,
                            limit,
                        } => (ContentType::Live, category, format, limit),
                        ListSubCommand::Movie {
                            category,
                            format,
                            limit,
                        }
                        | ListSubCommand::Movies {
                            category,
                            format,
                            limit,
                        }
                        | ListSubCommand::Vod {
                            category,
                            format,
                            limit,
                        } => (ContentType::Movie, category, format, limit),
                        ListSubCommand::Series {
                            category,
                            format,
                            limit,
                        }
                        | ListSubCommand::Tv {
                            category,
                            format,
                            limit,
                        } => (ContentType::Series, category, format, limit),
                    };

                    let output_format = OutputFormat::from_str(&format)?;
                    let cmd = ListCommand {
                        content_type,
                        category,
                        format: output_format,
                        limit,
                    };
                    cmd.execute(context).await?;
                }

                CliSubcommands::Info { id, r#type, format } => {
                    let content_type = ContentType::from_str(&r#type)?;
                    let output_format = OutputFormat::from_str(&format)?;
                    let cmd = InfoCommand {
                        id,
                        content_type,
                        format: output_format,
                    };
                    cmd.execute(context).await?;
                }

                CliSubcommands::Url { id, r#type } => {
                    let (api, _) = context.get_single_provider().await?;
                    let url = api.get_stream_url(id, &r#type, None);
                    println!("{}", url);
                }

                CliSubcommands::Fav(fav_cmd) => {
                    let cmd = match fav_cmd {
                        FavCommand::List { format } => {
                            let output_format = OutputFormat::from_str(&format)?;
                            FavoritesCommand::List {
                                format: output_format,
                            }
                        }
                        FavCommand::Add { id, r#type, name } => {
                            let content_type = ContentType::from_str(&r#type)?;
                            FavoritesCommand::Add {
                                id,
                                content_type,
                                name,
                            }
                        }
                        FavCommand::Remove { id } => FavoritesCommand::Remove { id },
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

                CliSubcommands::Providers(providers_cmd) => {
                    let cmd = match providers_cmd {
                        ProvidersSubCommand::List { format } => {
                            let output_format = OutputFormat::from_str(&format)?;
                            ProvidersCommand::List {
                                format: output_format,
                            }
                        }
                        ProvidersSubCommand::Test { name } => ProvidersCommand::Test { name },
                    };
                    cmd.execute(context).await?;
                }

                CliSubcommands::AddProvider => {
                    add_provider_interactively(config_path).await?;
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
