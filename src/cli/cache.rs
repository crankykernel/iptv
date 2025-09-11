use super::CommandContext;
use anyhow::Result;

pub enum CacheCommand {
    Refresh,
    Clear,
}

impl CacheCommand {
    pub async fn execute(self, context: CommandContext) -> Result<()> {
        let providers = context.get_providers().await?;

        match self {
            Self::Refresh => {
                for (mut api, provider_name) in providers {
                    // Force refresh the cache (clear then warm)
                    if let Err(e) = api.refresh_cache().await {
                        eprintln!(
                            "Warning: Failed to refresh cache for {}: {}",
                            provider_name, e
                        );
                    } else {
                        println!("\n✓ Cache refreshed for {}", provider_name);
                    }
                }
            }
            Self::Clear => {
                for (api, provider_name) in providers {
                    eprintln!("Clearing cache for {}...", provider_name);

                    if let Err(e) = api.cache_manager.clear_all_cache().await {
                        eprintln!(
                            "Warning: Failed to clear cache for {}: {}",
                            provider_name, e
                        );
                    } else {
                        println!("Cache cleared for {}", provider_name);
                    }
                }
            }
        }

        Ok(())
    }
}
