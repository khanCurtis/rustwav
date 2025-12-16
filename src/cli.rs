use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustwav")]
#[command(author = "Khanon Curtis")]
#[command(version = "0.1.0")]
#[command(about = "Rust-based music downloader")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Run in 3DS compatibility mode
    #[arg(long = "three-ds", default_value_t = false)]
    pub three-ds: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    Album {
        #[arg(short, long, default_value = "mp3")]
        format: String,
        #[arg(short, long, default_value = "high")]
        quality: String,
        link: String,
    },
    Playlist {
        #[arg(short, long, default_value = "mp3")]
        format: String,
        #[arg(short, long, default_value = "high")]
        quality: String,
        link: String,
    },
}

