use super::{CommandContext, OutputFormat};
use anyhow::Result;
use serde_json::json;

pub enum ProvidersCommand {
    List { format: OutputFormat },
    Test { name: Option<String> },
}

impl ProvidersCommand {
    pub async fn execute(self, context: CommandContext) -> Result<()> {
        match self {
            Self::List { format } => self.list_providers(context, format).await,
            Self::Test { ref name } => self.test_providers(context, name.clone()).await,
        }
    }

    async fn list_providers(&self, _context: CommandContext, format: OutputFormat) -> Result<()> {
        let providers_info: Vec<_> = _context
            .providers
            .iter()
            .map(|p| {
                json!({
                    "name": p.name.clone().unwrap_or_else(|| format!("{}@{}", p.username, p.url)),
                    "url": p.url,
                    "username": p.username,
                })
            })
            .collect();

        match format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&json!(providers_info))?);
            }
            OutputFormat::Text => {
                if providers_info.is_empty() {
                    println!("No providers configured");
                } else {
                    println!("Configured providers:");
                    for (i, info) in providers_info.iter().enumerate() {
                        if let Some(obj) = info.as_object() {
                            let name = obj["name"].as_str().unwrap_or("");
                            let url = obj["url"].as_str().unwrap_or("");
                            println!("  {}. {} ({})", i + 1, name, url);
                        }
                    }
                }
            }
            OutputFormat::M3u => {
                // M3U format doesn't make sense for provider list
                eprintln!("M3U format not supported for provider list");
                return Ok(());
            }
        }

        Ok(())
    }

    async fn test_providers(
        &self,
        mut context: CommandContext,
        name: Option<String>,
    ) -> Result<()> {
        let providers = if let Some(provider_name) = name {
            // Test specific provider
            context.selected_provider = Some(provider_name);
            context.get_providers().await?
        } else {
            // Test all providers
            context.get_all_providers().await?
        };

        for (mut api, provider_name) in providers {
            eprint!("Testing connection to {}... ", provider_name);

            match api.get_user_info().await {
                Ok(info) => {
                    println!("✓ Connected");
                    println!("  Account: {}", info.username);
                    println!("  Status: {}", info.status);
                    if !info.exp_date.is_empty() {
                        println!("  Expires: {}", info.exp_date);
                    }
                    println!("  Max connections: {}", info.max_connections);
                }
                Err(e) => {
                    println!("✗ Failed");
                    println!("  Error: {}", e);
                }
            }
        }

        Ok(())
    }
}
