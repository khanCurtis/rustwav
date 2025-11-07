use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustwav")]
#[command(author = "Khanon Curtis")]
#[command(version = "0.1.0")]
#[command(about = "Rust-based music downloader", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

pub enum Commands {
    /// Download an album
    Album {
        ///Spotify album link
        link: String,
        ///Audio format
        #[arg(short, long, default_value = "mp3")]
        format: String,
        ///Audio quality
        #[arg(short, long, default_value = "high")]
        quality: String,
    },
    /// Download a playlist
    Playlist {
        ///Spotify playlist link
        link: String,
        ///Audio format
        #[arg(short, long, default_value = "mp3")]
        format: String,
        ///Audio quality
        #[arg(short, long, default_value = "high")]
        quality: String,
    },
}
