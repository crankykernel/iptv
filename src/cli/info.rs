use super::{CommandContext, ContentType, OutputFormat};
use anyhow::Result;
use serde_json::json;

pub struct InfoCommand {
    pub id: u32,
    pub content_type: ContentType,
    pub format: OutputFormat,
}

impl InfoCommand {
    pub async fn execute(self, context: CommandContext) -> Result<()> {
        let (mut api, provider_name) = context.get_single_provider().await?;

        eprintln!("Fetching info from {}...", provider_name);

        let info = match self.content_type {
            ContentType::Movie => {
                let vod_info = api.get_vod_info(self.id).await?;
                json!({
                    "type": "movie",
                    "id": self.id,
                    "provider": provider_name,
                    "info": vod_info,
                })
            }
            ContentType::Series => {
                let series_info = api.get_series_info(self.id).await?;
                json!({
                    "type": "series",
                    "id": self.id,
                    "provider": provider_name,
                    "info": series_info.info,
                    "seasons": series_info.seasons,
                    "episodes": series_info.episodes,
                })
            }
            ContentType::Live => {
                // Live streams don't have detailed info, just get the stream
                let streams = api.get_live_streams(None).await?;
                let stream = streams
                    .iter()
                    .find(|s| s.stream_id == self.id)
                    .ok_or_else(|| anyhow::anyhow!("Stream {} not found", self.id))?;
                json!({
                    "type": "live",
                    "id": self.id,
                    "provider": provider_name,
                    "name": stream.name,
                    "category_id": stream.category_id,
                    "epg_channel_id": stream.epg_channel_id,
                })
            }
        };

        match self.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
            OutputFormat::Text => {
                if let Some(obj) = info.as_object() {
                    let content_type = obj["type"].as_str().unwrap_or("");
                    println!("Type: {}", content_type);
                    println!("ID: {}", self.id);
                    println!("Provider: {}", provider_name);

                    match self.content_type {
                        ContentType::Movie => {
                            if let Some(info) = obj["info"].as_object()
                                && let Some(movie_data) = info["movie_data"].as_object()
                            {
                                if let Some(name) = movie_data["name"].as_str() {
                                    println!("Title: {}", name);
                                }
                                if let Some(plot) = movie_data["plot"].as_str() {
                                    println!("\nPlot:\n{}", plot);
                                }
                                if let Some(cast) = movie_data["cast"].as_str() {
                                    println!("\nCast: {}", cast);
                                }
                                if let Some(director) = movie_data["director"].as_str() {
                                    println!("Director: {}", director);
                                }
                                if let Some(genre) = movie_data["genre"].as_str() {
                                    println!("Genre: {}", genre);
                                }
                                if let Some(rating) = movie_data["rating"].as_str() {
                                    println!("Rating: {}", rating);
                                }
                                if let Some(duration) = movie_data["duration"].as_str() {
                                    println!("Duration: {}", duration);
                                }
                            }
                        }
                        ContentType::Series => {
                            if let Some(info) = obj["info"].as_object() {
                                if let Some(name) = info["name"].as_str() {
                                    println!("Title: {}", name);
                                }
                                if let Some(plot) = info["plot"].as_str() {
                                    println!("\nPlot:\n{}", plot);
                                }
                                if let Some(cast) = info["cast"].as_str() {
                                    println!("\nCast: {}", cast);
                                }
                                if let Some(genre) = info["genre"].as_str() {
                                    println!("Genre: {}", genre);
                                }
                                if let Some(rating) = info["rating"].as_str() {
                                    println!("Rating: {}", rating);
                                }
                            }

                            if let Some(seasons) = obj["seasons"].as_array() {
                                println!("\nSeasons: {}", seasons.len());
                                for season in seasons {
                                    if let Some(s) = season.as_object() {
                                        let name = s["name"].as_str().unwrap_or("");
                                        let count = s["episode_count"].as_str().unwrap_or("0");
                                        println!("  {} ({} episodes)", name, count);
                                    }
                                }
                            }
                        }
                        ContentType::Live => {
                            if let Some(name) = obj["name"].as_str() {
                                println!("Name: {}", name);
                            }
                        }
                    }
                }
            }
            OutputFormat::M3u => {
                // M3U format doesn't make sense for detailed info
                eprintln!("M3U format not supported for info command, using text format instead");
                // Fall through to text output
                return Ok(());
            }
        }

        Ok(())
    }
}
