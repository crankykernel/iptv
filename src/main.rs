// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

use anyhow::Result;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};
use std::fs::File;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use iptv::config::ProviderConfig;
use iptv::xtream_api::XTreamAPI;
use iptv::{Config, Player};

mod commands;
use commands::{
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

    /// Command-line interface for scriptable operations
    Cli(CliCommands),

    /// Execute raw API calls
    #[command(subcommand)]
    Api(ApiCommand),
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

#[derive(Subcommand)]
enum ApiCommand {
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

async fn run_api_command(_provider: &str, api: &mut XTreamAPI, cmd: ApiCommand) -> Result<()> {
    let result = match cmd {
        ApiCommand::UserInfo => serde_json::to_value(api.get_user_info().await?)?,
        ApiCommand::LiveCategories => serde_json::to_value(api.get_live_categories().await?)?,
        ApiCommand::VodCategories => serde_json::to_value(api.get_vod_categories().await?)?,
        ApiCommand::SeriesCategories => serde_json::to_value(api.get_series_categories().await?)?,
        ApiCommand::LiveStreams { category } => {
            serde_json::to_value(api.get_live_streams(category.as_deref()).await?)?
        }
        ApiCommand::VodStreams { category } => {
            serde_json::to_value(api.get_vod_streams(category.as_deref()).await?)?
        }
        ApiCommand::Series { category } => {
            serde_json::to_value(api.get_series(category.as_deref()).await?)?
        }
        ApiCommand::SeriesInfo { id } => serde_json::to_value(api.get_series_info(id).await?)?,
        ApiCommand::VodInfo { id } => serde_json::to_value(api.get_vod_info(id).await?)?,
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

        Some(Commands::Api(api_cmd)) => {
            // Get provider from env var for API commands
            let selected_provider = std::env::var("IPTV_PROVIDER").ok();

            // Create command context
            let context = CommandContext::new(config.providers.clone(), selected_provider, false);

            let (mut api, provider_name) = context.get_single_provider().await?;
            eprintln!("Using provider: {}", provider_name);
            run_api_command(&provider_name, &mut api, api_cmd).await?;
        }
    }

    Ok(())
}
