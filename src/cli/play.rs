use super::{CommandContext, ContentType};
use anyhow::Result;
use iptv::Player;

pub struct PlayCommand {
    pub id: u32,
    pub content_type: Option<ContentType>,
    pub detached: bool,
}

impl PlayCommand {
    pub async fn execute(self, context: CommandContext, player: Player) -> Result<()> {
        let (mut api, provider_name) = context.get_single_provider().await?;

        eprintln!("Using provider: {}", provider_name);

        // Determine content type if not specified
        let content_type = if let Some(ct) = self.content_type {
            ct
        } else {
            // Try to auto-detect by checking different content types
            eprintln!("Auto-detecting content type...");

            // Check if it's a live stream
            if let Ok(streams) = api.get_live_streams(None).await {
                if streams.iter().any(|s| s.stream_id == self.id) {
                    ContentType::Live
                } else if let Ok(vods) = api.get_vod_streams(None).await {
                    if vods.iter().any(|s| s.stream_id == self.id) {
                        ContentType::Movie
                    } else {
                        anyhow::bail!("Stream ID {} not found", self.id);
                    }
                } else {
                    anyhow::bail!("Stream ID {} not found", self.id);
                }
            } else {
                anyhow::bail!("Failed to fetch streams");
            }
        };

        // Get the stream URL
        let url = api.get_stream_url(
            self.id,
            content_type.as_str(),
            None, // Extension will be auto-detected
        );

        eprintln!("Playing: {}", url);

        // Play the stream
        if self.detached {
            // Start MPV in detached mode - it will continue running after this command exits
            player.play_disassociated(&url).await?;
            println!("Stream started in detached window");
            println!("The player will continue running independently");
        } else {
            // Default: wait for MPV to exit (blocking)
            println!(
                "Starting playback... (Press 'q' in MPV to quit, or use --detached to run in background)"
            );
            player.play_blocking(&url).await?;
            println!("Playback ended");
        }

        Ok(())
    }
}
