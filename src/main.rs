mod extract_game_assets;

use anyhow::Result;
use clap::{Parser, Subcommand};
use extract_game_assets::{organize_3d_models, process_assets, validate_source};
use std::path::PathBuf;

use crate::extract_game_assets::organize_images;

#[derive(Parser, Debug)]
#[command(author, version, about = "Game asset manager CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Extract and copy assets from a source project
    Extract {
        #[arg(short, long)]
        source: PathBuf,

        #[arg(short, long)]
        destination: PathBuf,
    },
    /// Organize 3D models and their color maps into a root 3d_models folder
    Organize {
        #[arg(short, long)]
        dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Extract {
            source,
            destination,
        } => {
            validate_source(&source)?;
            println!("Scanning source: {:?}", source);
            println!("Target asset folder: {:?}", destination);

            let (copied_count, collision_count, duplicate_count, renamed_count) =
                process_assets(&source, &destination)?;

            println!("\nExtraction complete!");
            println!("- Files copied: {}", copied_count);
            println!("- Duplicates skipped: {}", duplicate_count);
            println!("- Collisions detected: {}", collision_count);
            println!("- Files renamed: {}", renamed_count);
        }
        Commands::Organize { dir } => {
            if !dir.exists() || !dir.is_dir() {
                anyhow::bail!(
                    "Assets directory does not exist or is not a directory: {:?}",
                    dir
                );
            }
            println!("Organizing 3D models and color maps in: {:?}", dir);
            let (models, textures) = organize_3d_models(&dir)?;

            println!("\nOrganization complete!");
            println!("- 3D models moved: {}", models);
            println!("- Color maps/textures moved: {}", textures);

            println!("Organizing images and XCF files in: {:?}", dir);
            let moved = organize_images(&dir)?;

            println!("\nImage organization complete!");
            println!("- Image/XCF files organized: {}", moved);
        }
    }

    Ok(())
}
