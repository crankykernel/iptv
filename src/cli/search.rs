use super::{CommandContext, ContentType, OutputFormat};
use anyhow::Result;
use serde_json::json;

pub struct SearchCommand {
    pub query: String,
    pub content_type: Option<ContentType>,
    pub format: OutputFormat,
}

impl SearchCommand {
    pub async fn execute(self, context: CommandContext) -> Result<()> {
        let providers = context.get_providers_for_search().await?;
        let query_lower = self.query.to_lowercase();

        // Check if we're searching multiple providers
        let is_multi_provider = providers.len() > 1;
        if is_multi_provider {
            eprintln!("Searching across {} providers...", providers.len());
        }

        let mut all_results = Vec::new();

        for (mut api, provider_name) in providers {
            if is_multi_provider {
                eprintln!("  Searching in {}...", provider_name);
            } else {
                eprintln!("Searching in {}...", provider_name);
            }

            let mut provider_results = Vec::new();

            // Search based on content type
            let search_types = if let Some(ct) = self.content_type {
                vec![ct]
            } else {
                vec![ContentType::Live, ContentType::Movie, ContentType::Series]
            };

            for content_type in search_types {
                match content_type {
                    ContentType::Live => {
                        if let Ok(streams) = api.get_live_streams(None).await {
                            for stream in streams {
                                if stream.name.to_lowercase().contains(&query_lower) {
                                    provider_results.push(json!({
                                        "id": stream.stream_id,
                                        "name": stream.name,
                                        "type": "live",
                                        "provider": &provider_name,
                                    }));
                                }
                            }
                        }
                    }
                    ContentType::Movie => {
                        if let Ok(streams) = api.get_vod_streams(None).await {
                            for stream in streams {
                                if stream.name.to_lowercase().contains(&query_lower) {
                                    provider_results.push(json!({
                                        "id": stream.stream_id,
                                        "name": stream.name,
                                        "type": "movie",
                                        "provider": &provider_name,
                                    }));
                                }
                            }
                        }
                    }
                    ContentType::Series => {
                        if let Ok(series) = api.get_series(None).await {
                            for s in series {
                                if s.name.to_lowercase().contains(&query_lower) {
                                    provider_results.push(json!({
                                        "id": s.series_id,
                                        "name": s.name,
                                        "type": "series",
                                        "provider": &provider_name,
                                    }));
                                }
                            }
                        }
                    }
                }
            }

            if is_multi_provider {
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
                    println!("No results found for '{}'", self.query);
                } else {
                    for result in all_results {
                        if let Some(obj) = result.as_object() {
                            if obj.contains_key("provider") && obj.contains_key("results") {
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
            let content_type = obj["type"].as_str().unwrap_or("");
            let provider = obj.get("provider").and_then(|p| p.as_str()).unwrap_or("");

            if provider.is_empty() {
                println!("[{}] {} ({})", content_type, name, id);
            } else {
                println!("[{}] {} ({}) - {}", content_type, name, id, provider);
            }
        }
    }

    fn print_m3u_entry(result: &serde_json::Value) {
        if let Some(obj) = result.as_object() {
            let id = obj["id"].as_u64().unwrap_or(0);
            let name = obj["name"].as_str().unwrap_or("");
            let content_type = obj["type"].as_str().unwrap_or("");

            // Note: Actual URL would need to be generated with proper auth
            println!("#EXTINF:-1,{}", name);
            println!("#EXTVLCOPT:type={}", content_type);
            println!("#EXTVLCOPT:id={}", id);
            println!("http://placeholder/{}/{}", content_type, id);
        }
    }
}
