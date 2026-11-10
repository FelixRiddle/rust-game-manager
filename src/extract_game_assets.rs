use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// List of supported file extensions to extract.
const SUPPORTED_EXTENSIONS: &[&str] =
    &["png", "xcf", "jpg", "blend", "blend1", "glb", "ogg", "ttf"];

/// Compares two files chunk-by-chunk to determine if they have identical contents.
///
/// # Arguments
/// * `path1` - Path to the first file.
/// * `path2` - Path to the second file.
pub fn files_are_identical(path1: &Path, path2: &Path) -> Result<bool> {
    let meta1 = fs::metadata(path1)?;
    let meta2 = fs::metadata(path2)?;

    // If file sizes differ, they cannot be identical
    if meta1.len() != meta2.len() {
        return Ok(false);
    }

    let f1 = File::open(path1)?;
    let f2 = File::open(path2)?;

    let mut reader1 = BufReader::new(f1);
    let mut reader2 = BufReader::new(f2);

    let mut buf1 = [0u8; 8192];
    let mut buf2 = [0u8; 8192];

    loop {
        let n1 = reader1.read(&mut buf1)?;
        let n2 = reader2.read(&mut buf2)?;

        if n1 != n2 || buf1[..n1] != buf2[..n2] {
            return Ok(false);
        }

        if n1 == 0 {
            break;
        }
    }

    Ok(true)
}

/// Validates that the provided source path exists and is a directory.
///
/// # Arguments
/// * `source` - A reference to the PathBuf representing the source directory.
pub fn validate_source(source: &PathBuf) -> Result<()> {
    if !source.exists() || !source.is_dir() {
        anyhow::bail!(
            "Source path does not exist or is not a directory: {:?}",
            source
        );
    }
    Ok(())
}

/// Converts a string into snake_case, handling camelCase, spaces, hyphens, and casing boundaries.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next = if i + 1 < chars.len() {
                    Some(chars[i + 1])
                } else {
                    None
                };

                // Insert underscore if previous was lowercase/number, or if next is lowercase (end of acronym)
                if prev.is_lowercase()
                    || prev.is_numeric()
                    || (prev.is_uppercase() && next.map_or(false, |n| n.is_lowercase()))
                {
                    result.push('_');
                }
            }
            result.push(c.to_ascii_lowercase());
        } else if c.is_alphanumeric() {
            result.push(c);
        } else {
            result.push('_');
        }
    }

    // Collapse multiple underscores and trim
    let parts: Vec<&str> = result.split('_').filter(|p| !p.is_empty()).collect();
    parts.join("_")
}

/// Transforms a relative path by converting every folder and the file stem to snake_case,
/// while keeping the file extension intact.
pub fn transform_path_to_snake_case(relative_path: &Path) -> PathBuf {
    let mut new_path = PathBuf::new();
    let components: Vec<_> = relative_path.components().collect();
    let len = components.len();

    for (i, component) in components.into_iter().enumerate() {
        if let std::path::Component::Normal(os_str) = component {
            if let Some(s) = os_str.to_str() {
                if i == len - 1 {
                    // Last component is a file; preserve its extension
                    let path_comp = PathBuf::from(s);
                    let stem = path_comp
                        .file_stem()
                        .and_then(|st| st.to_str())
                        .unwrap_or(s);
                    let ext = path_comp.extension().and_then(|e| e.to_str());

                    let mut new_filename = to_snake_case(stem);
                    if let Some(e) = ext {
                        new_filename.push('.');
                        new_filename.push_str(&e.to_lowercase());
                    }
                    new_path.push(new_filename);
                } else {
                    // Directory component
                    let dir_name = to_snake_case(s);
                    new_path.push(dir_name);
                }
            } else {
                new_path.push(component);
            }
        } else {
            new_path.push(component);
        }
    }
    new_path
}

/// Recursively scans the source directory, filters by supported extensions,
/// converts path components to snake_case, deduplicates identical files, and handles collisions.
pub fn process_assets(source: &PathBuf, destination: &PathBuf) -> Result<(usize, usize, usize)> {
    let mut copied_count = 0;
    let mut collision_count = 0;
    let mut duplicate_count = 0;

    for entry in WalkDir::new(source).into_iter().filter_map(Result::ok) {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    let relative_path = path
                        .strip_prefix(source)
                        .context("Failed to strip prefix from source path")?;

                    let snake_relative_path = transform_path_to_snake_case(relative_path);
                    let target_path = destination.join(&snake_relative_path);

                    if target_path.exists() {
                        // Check if the existing file is an exact duplicate (same content)
                        if files_are_identical(path, &target_path)? {
                            println!(
                                "[DUPLICATE] Skipping identical file: {:?}",
                                snake_relative_path
                            );
                            duplicate_count += 1;
                            continue;
                        } else {
                            println!(
                                "[COLLISION] File exists with different content: {:?}",
                                snake_relative_path
                            );
                            collision_count += 1;
                        }
                    }

                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent)
                            .context(format!("Failed to create directory: {:?}", parent))?;
                    }

                    fs::copy(path, &target_path).context(format!(
                        "Failed to copy file from {:?} to {:?}",
                        path, target_path
                    ))?;

                    println!("Copied: {:?}", snake_relative_path);
                    copied_count += 1;
                }
            }
        }
    }

    Ok((copied_count, collision_count, duplicate_count))
}
