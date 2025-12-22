use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustwav")]
#[command(author = "Khanon Curtis")]
#[command(version = "0.1.0")]
#[command(about = "Rust-based music downloader")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Run in portable mode (constrained devices: 3DS, car stereos, old MP3 players)
    /// Forces MP3 format, FAT32-safe filenames, shallow folders, small cover art
    #[arg(long = "portable", short = 'p', default_value_t = false, global = true)]
    pub portable: bool,
}

/// Runtime configuration derived from CLI flags
#[derive(Clone, Debug)]
pub struct PortableConfig {
    pub enabled: bool,
    pub max_cover_dim: u32,
    pub max_cover_bytes: usize,
    pub max_filename_len: usize,
}

impl PortableConfig {
    pub fn from_cli(cli: &Cli) -> Self {
        if cli.portable {
            Self {
                enabled: true,
                max_cover_dim: 128,
                max_cover_bytes: 64 * 1024,
                max_filename_len: 64,
            }
        } else {
            Self {
                enabled: false,
                max_cover_dim: 500,
                max_cover_bytes: 300 * 1024,
                max_filename_len: 100,
            }
        }
    }
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

