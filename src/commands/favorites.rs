use super::{CommandContext, ContentType, OutputFormat};
use anyhow::Result;
use iptv::xtream_api::FavouriteStream;
use serde_json::json;

pub enum FavoritesCommand {
    List {
        format: OutputFormat,
    },
    Add {
        id: u32,
        content_type: ContentType,
        name: Option<String>,
    },
    Remove {
        id: u32,
    },
}

impl FavoritesCommand {
    pub async fn execute(self, context: CommandContext) -> Result<()> {
        match self {
            Self::List { format } => self.list_favorites(context, format).await,
            Self::Add {
                id,
                content_type,
                ref name,
            } => {
                self.add_favorite(context, id, content_type, name.clone())
                    .await
            }
            Self::Remove { id } => self.remove_favorite(context, id).await,
        }
    }

    async fn list_favorites(&self, context: CommandContext, format: OutputFormat) -> Result<()> {
        let providers = context.get_providers().await?;
        let mut all_favorites = Vec::new();

        for (api, provider_name) in providers {
            let favorites = api
                .favourites_manager
                .get_favourites(&api.provider_hash)
                .unwrap_or_default();

            if context.all_providers {
                all_favorites.push(json!({
                    "provider": provider_name,
                    "favorites": favorites.iter().map(|f| json!({
                        "id": f.stream_id,
                        "name": f.name,
                        "type": f.stream_type,
                    })).collect::<Vec<_>>(),
                }));
            } else {
                all_favorites.extend(favorites.iter().map(|f| {
                    json!({
                        "id": f.stream_id,
                        "name": f.name,
                        "type": f.stream_type,
                        "provider": &provider_name,
                    })
                }));
            }
        }

        match format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&json!(all_favorites))?);
            }
            OutputFormat::Text => {
                if all_favorites.is_empty() {
                    println!("No favorites found");
                } else {
                    for fav in all_favorites {
                        if let Some(obj) = fav.as_object() {
                            if context.all_providers && obj.contains_key("provider") {
                                println!("\n{}:", obj["provider"].as_str().unwrap_or(""));
                                if let Some(favs) = obj["favorites"].as_array() {
                                    for f in favs {
                                        if let Some(fobj) = f.as_object() {
                                            let id = fobj["id"].as_u64().unwrap_or(0);
                                            let name = fobj["name"].as_str().unwrap_or("");
                                            let ftype = fobj["type"].as_str().unwrap_or("");
                                            println!("  [{:6}] {} ({})", id, name, ftype);
                                        }
                                    }
                                }
                            } else {
                                let id = obj["id"].as_u64().unwrap_or(0);
                                let name = obj["name"].as_str().unwrap_or("");
                                let ftype = obj["type"].as_str().unwrap_or("");
                                println!("[{:6}] {} ({})", id, name, ftype);
                            }
                        }
                    }
                }
            }
            OutputFormat::M3u => {
                println!("#EXTM3U");
                for fav in all_favorites {
                    if let Some(obj) = fav.as_object() {
                        if obj.contains_key("favorites") {
                            if let Some(favs) = obj["favorites"].as_array() {
                                for f in favs {
                                    Self::print_m3u_favorite(f);
                                }
                            }
                        } else {
                            Self::print_m3u_favorite(&fav);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn add_favorite(
        &self,
        context: CommandContext,
        id: u32,
        content_type: ContentType,
        name: Option<String>,
    ) -> Result<()> {
        let (mut api, provider_name) = context.get_single_provider().await?;

        // Get the stream name if not provided
        let stream_name = if let Some(n) = name {
            n
        } else {
            // Fetch the stream to get its name
            match content_type {
                ContentType::Live => {
                    let streams = api.get_live_streams(None).await?;
                    streams
                        .iter()
                        .find(|s| s.stream_id == id)
                        .map(|s| s.name.clone())
                        .ok_or_else(|| anyhow::anyhow!("Stream {} not found", id))?
                }
                ContentType::Movie => {
                    let streams = api.get_vod_streams(None).await?;
                    streams
                        .iter()
                        .find(|s| s.stream_id == id)
                        .map(|s| s.name.clone())
                        .ok_or_else(|| anyhow::anyhow!("Movie {} not found", id))?
                }
                ContentType::Series => {
                    let series = api.get_series(None).await?;
                    series
                        .iter()
                        .find(|s| s.series_id == id)
                        .map(|s| s.name.clone())
                        .ok_or_else(|| anyhow::anyhow!("Series {} not found", id))?
                }
            }
        };

        let favorite = FavouriteStream {
            stream_id: id,
            name: stream_name.clone(),
            stream_type: content_type.as_str().to_string(),
            provider_hash: api.provider_hash.clone(),
            added_date: chrono::Utc::now(),
            category_id: None,
        };

        api.favourites_manager
            .add_favourite(&api.provider_hash, favorite)?;

        println!("Added '{}' to favorites in {}", stream_name, provider_name);
        Ok(())
    }

    async fn remove_favorite(&self, context: CommandContext, id: u32) -> Result<()> {
        let (api, provider_name) = context.get_single_provider().await?;

        // Get current favorites to find the type
        let favorites = api
            .favourites_manager
            .get_favourites(&api.provider_hash)
            .unwrap_or_default();

        let favorite = favorites
            .iter()
            .find(|f| f.stream_id == id)
            .ok_or_else(|| anyhow::anyhow!("Favorite {} not found", id))?;

        let stream_type = &favorite.stream_type;
        let name = &favorite.name;

        api.favourites_manager
            .remove_favourite(&api.provider_hash, id, stream_type)?;

        println!("Removed '{}' from favorites in {}", name, provider_name);
        Ok(())
    }

    fn print_m3u_favorite(fav: &serde_json::Value) {
        if let Some(obj) = fav.as_object() {
            let id = obj["id"].as_u64().unwrap_or(0);
            let name = obj["name"].as_str().unwrap_or("");
            let ftype = obj["type"].as_str().unwrap_or("");

            println!("#EXTINF:-1,{}", name);
            println!("http://placeholder/{}/{}", ftype, id);
        }
    }
}
