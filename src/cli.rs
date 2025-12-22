use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "rustwav")]
#[command(author = "Khanon Curtis")]
#[command(version = "0.1.0")]
#[command(about = "Rust-based music downloader")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Run in portable mode (constrained devices: 3DS, car stereos, old MP3 players)
    #[arg(long = "portable", short = 'p', default_value_t = false)]
    pub portable: bool,

    /// Run in headless mode (no prompts, script-friendly output)
    #[arg(long = "headless", short = 'H', default_value_t = false)]
    pub headless: bool,

    /// Output format for headless mode
    #[arg(long = "output", short = 'o', value_enum, default_value_t = OutputFormat::Text)]
    pub output_format: OutputFormat,

    /// Quiet mode - only show errors
    #[arg(long = "quiet", short = 'q', default_value_t = false)]
    pub quiet: bool,

    /// Verbose mode - show debug information
    #[arg(long = "verbose", short = 'v', default_value_t = false)]
    pub verbose: bool,
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
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

/// Headless mode configuration
#[derive(Clone, Debug)]
pub struct HeadlessConfig {
    pub enabled: bool,
    pub output_format: OutputFormat,
    pub quiet: bool,
    pub verbose: bool,
}

impl HeadlessConfig {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            enabled: cli.headless,
            output_format: cli.output_format.clone(),
            quiet: cli.quiet,
            verbose: cli.verbose,
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

