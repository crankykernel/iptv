use super::{CommandContext, ContentType, OutputFormat};
use anyhow::Result;
use serde_json::json;

pub struct ListCommand {
    pub content_type: ContentType,
    pub category: Option<String>,
    pub format: OutputFormat,
    pub limit: Option<usize>,
}

impl ListCommand {
    pub async fn execute(self, context: CommandContext) -> Result<()> {
        let providers = context.get_providers().await?;
        let mut all_results = Vec::new();

        for (mut api, provider_name) in providers {
            eprintln!("Fetching from {}...", provider_name);

            let provider_results = match self.content_type {
                ContentType::Live => {
                    let streams = api.get_live_streams(self.category.as_deref()).await?;
                    streams
                        .into_iter()
                        .take(self.limit.unwrap_or(usize::MAX))
                        .map(|s| {
                            json!({
                                "id": s.stream_id,
                                "name": s.name,
                                "type": "live",
                                "category_id": s.category_id,
                                "provider": &provider_name,
                            })
                        })
                        .collect::<Vec<_>>()
                }
                ContentType::Movie => {
                    let streams = api.get_vod_streams(self.category.as_deref()).await?;
                    streams
                        .into_iter()
                        .take(self.limit.unwrap_or(usize::MAX))
                        .map(|s| {
                            json!({
                                "id": s.stream_id,
                                "name": s.name,
                                "type": "movie",
                                "category_id": s.category_id,
                                "rating": s.rating,
                                "provider": &provider_name,
                            })
                        })
                        .collect::<Vec<_>>()
                }
                ContentType::Series => {
                    let series = api.get_series(self.category.as_deref()).await?;
                    series
                        .into_iter()
                        .take(self.limit.unwrap_or(usize::MAX))
                        .map(|s| {
                            json!({
                                "id": s.series_id,
                                "name": s.name,
                                "type": "series",
                                "category_id": s.category_id,
                                "rating": s.rating,
                                "provider": &provider_name,
                            })
                        })
                        .collect::<Vec<_>>()
                }
            };

            if context.all_providers {
                all_results.push(json!({
                    "provider": provider_name,
                    "results": provider_results,
                }));
            } else {
                all_results.extend(provider_results);
            }
        }

        // Output results in requested format
        match self.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&json!(all_results))?);
            }
            OutputFormat::Text => {
                if all_results.is_empty() {
                    println!("No {} found", self.content_type.as_str());
                } else {
                    for result in all_results {
                        if let Some(obj) = result.as_object() {
                            if context.all_providers && obj.contains_key("provider") {
                                // Multi-provider format
                                println!("\n{}:", obj["provider"].as_str().unwrap_or(""));
                                if let Some(results) = obj["results"].as_array() {
                                    for r in results {
                                        Self::print_text_result(r);
                                    }
                                }
                            } else {
                                // Single result
                                Self::print_text_result(&result);
                            }
                        }
                    }
                }
            }
            OutputFormat::M3u => {
                println!("#EXTM3U");
                println!("#EXTM3U x-tvg-url=\"\"");
                for result in all_results {
                    if let Some(obj) = result.as_object() {
                        if obj.contains_key("results") {
                            // Multi-provider format
                            if let Some(results) = obj["results"].as_array() {
                                for r in results {
                                    Self::print_m3u_entry(r);
                                }
                            }
                        } else {
                            // Single result
                            Self::print_m3u_entry(&result);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn print_text_result(result: &serde_json::Value) {
        if let Some(obj) = result.as_object() {
            let id = obj["id"].as_u64().unwrap_or(0);
            let name = obj["name"].as_str().unwrap_or("");
            println!("{:6} | {}", id, name);
        }
    }

    fn print_m3u_entry(result: &serde_json::Value) {
        if let Some(obj) = result.as_object() {
            let id = obj["id"].as_u64().unwrap_or(0);
            let name = obj["name"].as_str().unwrap_or("");
            let content_type = obj["type"].as_str().unwrap_or("");

            println!(
                "#EXTINF:-1 tvg-id=\"{}\" tvg-name=\"{}\",{}",
                id, name, name
            );
            println!("http://placeholder/{}/{}", content_type, id);
        }
    }
}
