use anyhow::Result;
use inquire::Select;
use serde::{Deserialize, Serialize};

use iptv::config::ProviderConfig;
use iptv::xtream::XTreamAPI;

pub mod cache;
pub mod search;

pub use cache::CacheCommand;
pub use search::SearchCommand;

/// Output format for command results
#[derive(Debug, Clone, Copy)]
pub enum OutputFormat {
    Text,
    Json,
    M3u,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "m3u" => Ok(Self::M3u),
            _ => anyhow::bail!("Invalid format: {}. Use 'text', 'json', or 'm3u'", s),
        }
    }
}

/// Context for command execution with provider management
pub struct CommandContext {
    pub providers: Vec<ProviderConfig>,
    pub selected_provider: Option<String>,
    pub all_providers: bool,
}

impl CommandContext {
    pub fn new(
        providers: Vec<ProviderConfig>,
        selected_provider: Option<String>,
        all_providers: bool,
    ) -> Self {
        Self {
            providers,
            selected_provider,
            all_providers,
        }
    }

    /// Get a single provider for commands that require exactly one
    pub async fn get_single_provider(&self) -> Result<(XTreamAPI, String)> {
        if self.providers.is_empty() {
            anyhow::bail!("No providers configured. Please add provider details to config.toml.");
        }

        let provider = if let Some(name) = &self.selected_provider {
            // Find provider by name (case-insensitive)
            self.providers
                .iter()
                .find(|p| {
                    p.name
                        .as_ref()
                        .map(|n| n.to_lowercase() == name.to_lowercase())
                        .unwrap_or(false)
                })
                .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", name))?
        } else if self.providers.len() == 1 {
            // Auto-select single provider
            &self.providers[0]
        } else {
            // Multiple providers, need selection
            self.prompt_provider_selection()?
        };

        let provider_name = provider
            .name
            .clone()
            .unwrap_or_else(|| format!("{}@{}", provider.username, provider.url));

        let api = XTreamAPI::new_with_id(
            provider.url.clone(),
            provider.username.clone(),
            provider.password.clone(),
            Some(provider_name.clone()),
            provider.id.clone(),
        )?;

        Ok((api, provider_name))
    }

    /// Get all providers for commands that can work across multiple
    pub async fn get_all_providers(&self) -> Result<Vec<(XTreamAPI, String)>> {
        let mut apis = Vec::new();

        for provider in &self.providers {
            let provider_name = provider
                .name
                .clone()
                .unwrap_or_else(|| format!("{}@{}", provider.username, provider.url));

            let api = XTreamAPI::new_with_id(
                provider.url.clone(),
                provider.username.clone(),
                provider.password.clone(),
                Some(provider_name.clone()),
                provider.id.clone(),
            )?;

            apis.push((api, provider_name));
        }

        Ok(apis)
    }

    /// Get providers based on context (single, all, or selected)
    pub async fn get_providers(&self) -> Result<Vec<(XTreamAPI, String)>> {
        if self.all_providers && !self.providers.is_empty() {
            self.get_all_providers().await
        } else {
            let provider = self.get_single_provider().await?;
            Ok(vec![provider])
        }
    }

    /// Get providers for search - defaults to all if no specific provider selected
    pub async fn get_providers_for_search(&self) -> Result<Vec<(XTreamAPI, String)>> {
        // If a specific provider is selected, use only that one
        if self.selected_provider.is_some() {
            let provider = self.get_single_provider().await?;
            Ok(vec![provider])
        } else if self.providers.len() == 1 {
            // Single provider, use it
            let provider = self.get_single_provider().await?;
            Ok(vec![provider])
        } else if !self.providers.is_empty() {
            // Multiple providers and no specific one selected - search all
            self.get_all_providers().await
        } else {
            anyhow::bail!("No providers configured")
        }
    }

    /// Prompt user to select a provider
    fn prompt_provider_selection(&self) -> Result<&ProviderConfig> {
        let provider_names: Vec<String> = self
            .providers
            .iter()
            .map(|p| {
                p.name
                    .clone()
                    .unwrap_or_else(|| format!("{}@{}", p.username, p.url))
            })
            .collect();

        let selection = Select::new("Select provider:", provider_names).prompt()?;

        // Find the provider by matching the display name
        self.providers
            .iter()
            .find(|p| {
                let name = p
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("{}@{}", p.username, p.url));
                name == selection
            })
            .ok_or_else(|| anyhow::anyhow!("Provider not found"))
    }
}

/// Content type for filtering
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ContentType {
    Live,
    Movie,
    Series,
}

impl ContentType {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "live" => Ok(Self::Live),
            "movie" | "movies" | "vod" => Ok(Self::Movie),
            "series" | "tv" => Ok(Self::Series),
            _ => anyhow::bail!("Invalid type: {}. Use 'live', 'movie', or 'series'", s),
        }
    }
}
