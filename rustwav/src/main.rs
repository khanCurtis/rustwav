mod cli;
mod config;
mod db;
mod downloader;
mod file_utils;
mod metadata;
mod sources;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        cli::Commands::Album {
            link,
            format,
            quality,
        } => {
            println!("Fetching album metadata for: {}", link);
            sources::spotify::fetch_album(link).await?;
            // Placeholder for downloader call
        }
        cli::Commands::Playlist {
            link,
            format,
            quality,
        } => {
            println!("Fetching playlist metadata for: {}", link);
            sources::spotify::fetch_playlist(link).await?;
        }
    }

    Ok(())
}
