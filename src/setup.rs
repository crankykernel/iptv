use anyhow::Result;
use inquire::validator::Validation;
use inquire::{Confirm, Text};
use std::path::Path;

use crate::config::{Config, ProviderConfig, Settings};

pub async fn interactive_provider_setup() -> Result<()> {
    println!("\n🚀 Welcome to IPTV! Let's set up your first provider.\n");

    println!("This app requires an IPTV provider that supports the Xtream Codes API.");
    println!("You'll need:");
    println!("  • The provider's server URL");
    println!("  • Your username");
    println!("  • Your password\n");

    let add_provider = Confirm::new("Would you like to add a provider now?")
        .with_default(true)
        .prompt()?;

    if !add_provider {
        println!("\nYou can add a provider later by editing the config file at:");
        println!("  ~/.config/iptv/config.toml");
        return Ok(());
    }

    let provider = prompt_for_provider().await?;

    let mut config = Config {
        providers: vec![provider],
        settings: Settings::default(),
    };

    let add_another = Confirm::new("Would you like to add another provider?")
        .with_default(false)
        .prompt()?;

    if add_another {
        loop {
            let provider = prompt_for_provider().await?;
            config.providers.push(provider);

            let continue_adding = Confirm::new("Add another provider?")
                .with_default(false)
                .prompt()?;

            if !continue_adding {
                break;
            }
        }
    }

    save_config(&config)?;

    println!("\n✅ Configuration saved successfully!");
    println!("You can now:");
    println!("  • Run 'iptv --tui' to launch the interactive TUI");
    println!("  • Run 'iptv rofi' to launch the rofi menu (requires rofi)");
    println!("  • Edit your config at ~/.config/iptv/config.toml");

    Ok(())
}

async fn prompt_for_provider() -> Result<ProviderConfig> {
    println!("\n📝 Provider Configuration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━");

    let name = Text::new("Provider name (optional):")
        .with_help_message("A friendly name for this provider")
        .prompt_skippable()?;

    let url = Text::new("Server URL:")
        .with_help_message("e.g., https://your-server.com:port/player_api.php")
        .with_validator(|input: &str| {
            if input.is_empty() {
                Ok(Validation::Invalid("Server URL is required".into()))
            } else if !input.starts_with("http://") && !input.starts_with("https://") {
                Ok(Validation::Invalid(
                    "URL must start with http:// or https://".into(),
                ))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt()?;

    let username = Text::new("Username:")
        .with_validator(|input: &str| {
            if input.is_empty() {
                Ok(Validation::Invalid("Username is required".into()))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt()?;

    let password = Text::new("Password:")
        .with_validator(|input: &str| {
            if input.is_empty() {
                Ok(Validation::Invalid("Password is required".into()))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt()?;

    println!("\nTesting connection...");

    if let Err(e) = test_provider_connection(&url, &username, &password).await {
        println!("⚠️  Warning: Could not verify connection: {}", e);
        println!(
            "    The provider will be saved anyway, but you may need to check your credentials."
        );
    } else {
        println!("✅ Connection successful!");
    }

    Ok(ProviderConfig {
        id: None,
        name,
        url,
        username,
        password,
    })
}

async fn test_provider_connection(url: &str, username: &str, password: &str) -> Result<()> {
    use crate::xtream::XTreamAPI;

    let mut api = XTreamAPI::new(
        url.to_string(),
        username.to_string(),
        password.to_string(),
        None,
    )?;

    // Try to refresh cache - this will fetch categories which most providers support
    match tokio::time::timeout(std::time::Duration::from_secs(10), api.refresh_cache()).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(anyhow::anyhow!("Failed to connect: {}", e)),
        Err(_) => Err(anyhow::anyhow!("Connection timeout")),
    }
}

fn save_config(config: &Config) -> Result<()> {
    let config_dir = Config::ensure_config_dir()?;
    let config_path = config_dir.join("config.toml");

    if config_path.exists() {
        let backup_path = config_dir.join("config.toml.backup");
        std::fs::copy(&config_path, &backup_path)?;
        println!(
            "ℹ️  Existing config backed up to: {}",
            backup_path.display()
        );
    }

    config.save(&config_path)?;
    println!("💾 Configuration saved to: {}", config_path.display());

    Ok(())
}

pub fn should_run_setup(config_path: &Path, config: &Config) -> bool {
    !config_path.exists()
        || config.providers.is_empty()
        || (config.providers.len() == 1
            && config.providers[0].url == "https://your-server.com:port/player_api.php"
            && config.providers[0].username == "your-username")
}
