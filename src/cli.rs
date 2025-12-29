use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustwav")]
#[command(author = "Khanon Curtis")]
#[command(version = "0.1.0")]
#[command(about = "Rust-based music downloader")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Run in portable mode (constrained devices: 3DS, car stereos, old MP3 players)
    /// Forces MP3 format, FAT32-safe filenames, shallow folders, small cover art
    #[arg(long = "portable", short = 'p', default_value_t = false)]
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
    /// Convert audio files between formats (mp3, flac, wav, aac)
    Convert {
        /// Input file or directory to convert
        #[arg(short, long)]
        input: String,

        /// Target format (mp3, flac, wav, aac)
        #[arg(short = 't', long, default_value = "mp3")]
        to: String,

        /// Quality for lossy formats (high, medium, low)
        #[arg(short, long, default_value = "high")]
        quality: String,

        /// Refresh metadata from Spotify after conversion
        #[arg(long, default_value_t = true)]
        refresh_metadata: bool,

        /// Process directories recursively
        #[arg(short, long, default_value_t = false)]
        recursive: bool,
    },
    /// Clean up the download database by removing entries for deleted files
    Cleanup {
        /// Show what would be removed without actually removing (dry run)
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Show detailed list of removed entries
        #[arg(short, long, default_value_t = false)]
        verbose: bool,
    },
}

