use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CategoryIgnores {
    #[serde(default)]
    pub live: HashSet<String>,
    #[serde(default)]
    pub movies: HashSet<String>,
    #[serde(default)]
    pub series: HashSet<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProviderIgnores {
    #[serde(default)]
    pub categories: CategoryIgnores,
    // Future: could add ignored streams, ignored series, etc.
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct IgnoredCategories {
    // Structure: provider_name -> ignore types -> content_type -> Set of names
    // This creates a nice nested JSON structure:
    // {
    //   "providers": {
    //     "MegaOTT": {
    //       "categories": {
    //         "live": ["News", "Sports"],
    //         "movies": ["Horror"],
    //         "series": ["Reality TV"]
    //       }
    //     }
    //   }
    // }
    #[serde(default)]
    providers: HashMap<String, ProviderIgnores>,
}

impl IgnoredCategories {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn toggle_category(
        &mut self,
        provider: &str,
        content_type: &str,
        category: &str,
    ) -> Result<bool> {
        let provider_ignores = self.providers.entry(provider.to_string()).or_default();

        let categories = match content_type {
            "live" => &mut provider_ignores.categories.live,
            "movies" => &mut provider_ignores.categories.movies,
            "series" => &mut provider_ignores.categories.series,
            _ => return Err(anyhow::anyhow!("Invalid content type: {}", content_type)),
        };

        let is_ignored = if categories.contains(category) {
            categories.remove(category);
            false
        } else {
            categories.insert(category.to_string());
            true
        };

        self.save()?;
        Ok(is_ignored)
    }

    pub fn is_ignored(&self, provider: &str, content_type: &str, category: &str) -> bool {
        self.providers
            .get(provider)
            .map(|provider_ignores| match content_type {
                "live" => provider_ignores.categories.live.contains(category),
                "movies" => provider_ignores.categories.movies.contains(category),
                "series" => provider_ignores.categories.series.contains(category),
                _ => false,
            })
            .unwrap_or(false)
    }

    pub fn get_ignored_for_provider(&self, provider: &str, content_type: &str) -> HashSet<String> {
        self.providers
            .get(provider)
            .map(|provider_ignores| match content_type {
                "live" => provider_ignores.categories.live.clone(),
                "movies" => provider_ignores.categories.movies.clone(),
                "series" => provider_ignores.categories.series.clone(),
                _ => HashSet::new(),
            })
            .unwrap_or_default()
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("iptv").join("ignored.json"))
    }
}
