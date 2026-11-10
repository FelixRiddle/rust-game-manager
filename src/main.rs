use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod extract_game_assets;

/// Game Manager CLI for organizing and extracting project assets.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Manages and extracts game assets while preserving folder hierarchy.",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for the game manager tool.
#[derive(Subcommand, Debug)]
enum Commands {
    /// Extract and copy assets from a source project to a destination folder.
    Extract {
        /// Path to the source project directory (e.g., Godot project root).
        #[arg(short, long)]
        source: PathBuf,

        /// Path to the destination asset folder.
        #[arg(short, long)]
        destination: PathBuf,
    },
}

/// Main entry point for the game manager application.
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Extract {
            source,
            destination,
        } => {
            extract_game_assets::validate_source(&source)?;

            println!("Scanning source: {:?}", source);
            println!("Target asset folder: {:?}", destination);

            let (copied_count, collision_count) =
                extract_game_assets::process_assets(&source, &destination)?;

            println!("\nExtraction complete!");
            println!("- Files copied: {}", copied_count);
            println!("- Collisions detected: {}", collision_count);
        }
    }

    Ok(())
}
