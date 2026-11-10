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
/// converts path components to snake_case, deduplicates identical files,
/// handles collisions by renaming with a counter, and tracks renamed counts.
pub fn process_assets(
    source: &PathBuf,
    destination: &PathBuf,
) -> Result<(usize, usize, usize, usize)> {
    let mut copied_count = 0;
    let mut collision_count = 0;
    let mut duplicate_count = 0;
    let mut renamed_count = 0;

    for entry in WalkDir::new(source).into_iter().filter_map(Result::ok) {
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                    let relative_path = path
                        .strip_prefix(source)
                        .context("Failed to strip prefix from source path")?;

                    let snake_relative_path = transform_path_to_snake_case(relative_path);
                    let mut target_path = destination.join(&snake_relative_path);
                    let mut is_renamed = false;

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

                            // Resolve collision by finding a unique filename using a counter
                            let parent_dir = target_path.parent().unwrap_or(destination);
                            let file_stem = target_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("file");
                            let extension = target_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("");

                            let mut counter = 1;
                            loop {
                                let new_filename = if extension.is_empty() {
                                    format!("{}_{}", file_stem, counter)
                                } else {
                                    format!("{}_{}.{}", file_stem, counter, extension)
                                };

                                let candidate_path = parent_dir.join(new_filename);
                                if !candidate_path.exists() {
                                    target_path = candidate_path;
                                    is_renamed = true;
                                    break;
                                }

                                if files_are_identical(path, &candidate_path)? {
                                    println!(
                                        "[DUPLICATE] Skipping identical file found at renamed path: {:?}",
                                        candidate_path
                                    );
                                    duplicate_count += 1;
                                    is_renamed = false;
                                    break;
                                }
                                counter += 1;
                            }

                            if !is_renamed
                                && files_are_identical(path, &target_path).unwrap_or(false)
                            {
                                continue;
                            }
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

                    if is_renamed {
                        println!("Copied & Renamed to: {:?}", target_path);
                        renamed_count += 1;
                    } else {
                        println!("Copied: {:?}", snake_relative_path);
                    }
                    copied_count += 1;
                }
            }
        }
    }

    Ok((
        copied_count,
        collision_count,
        duplicate_count,
        renamed_count,
    ))
}

/// Scans the assets root directory (strictly ignoring the `3d_models` target folder itself)
/// for all 3D models and their associated color maps, creating a dedicated subfolder for
/// each model inside `3d_models` using both the parent folder and model name to avoid generic collisions.
pub fn organize_3d_models(assets_root: &PathBuf) -> Result<(usize, usize)> {
    let target_dir = assets_root.join("3d_models");
    fs::create_dir_all(&target_dir).context(format!(
        "Failed to create root 3d_models directory: {:?}",
        target_dir
    ))?;

    let model_extensions = ["blend", "blend1", "glb"];
    let texture_extensions = ["png", "jpg", "jpeg"];

    let mut moved_models = 0;
    let mut moved_textures = 0;

    let mut model_paths = Vec::new();

    // Explicitly skip walking into the '3d_models' target folder during scanning
    for entry in WalkDir::new(assets_root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| e.path() != target_dir)
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if model_extensions.contains(&ext.to_lowercase().as_str()) {
                    model_paths.push(path.to_path_buf());
                }
            }
        }
    }

    for model_path in model_paths {
        if let Some(parent) = model_path.parent() {
            if parent == target_dir || !parent.exists() {
                continue;
            }

            if parent.is_dir() {
                let model_stem = match model_path.file_stem().and_then(|s| s.to_str()) {
                    Some(stem) => to_snake_case(stem),
                    None => continue,
                };

                // Get parent folder name to disambiguate generic names like "model"
                let parent_folder_name = match parent.file_name().and_then(|n| n.to_str()) {
                    Some(name) => to_snake_case(name),
                    None => "model".to_string(),
                };

                // If the model is named generically (e.g. model, mesh), prefix it with the parent folder name
                let subfolder_name = if model_stem == "model" || model_stem == "mesh" {
                    format!("{}_{}", parent_folder_name, model_stem)
                } else {
                    model_stem.clone()
                };

                // Create a dedicated subfolder under 3d_models
                let model_subfolder = target_dir.join(&subfolder_name);
                fs::create_dir_all(&model_subfolder).context(format!(
                    "Failed to create model subfolder: {:?}",
                    model_subfolder
                ))?;

                let entries = match fs::read_dir(parent) {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                for entry in entries {
                    let entry = entry?;
                    let file_path = entry.path();
                    if file_path.is_file() {
                        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                            let ext_lower = ext.to_lowercase();
                            let is_model = model_extensions.contains(&ext_lower.as_str());
                            let is_texture = texture_extensions.contains(&ext_lower.as_str());

                            if is_model || is_texture {
                                let file_stem = file_path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("file");
                                let snake_stem = to_snake_case(file_stem);
                                let formatted_filename = format!("{}.{}", snake_stem, ext_lower);

                                let dest_file_path = model_subfolder.join(&formatted_filename);

                                // Idempotency check
                                if dest_file_path.exists() {
                                    if files_are_identical(&file_path, &dest_file_path)
                                        .unwrap_or(false)
                                    {
                                        let _ = fs::remove_file(&file_path);
                                        continue;
                                    } else {
                                        let mut counter = 1;
                                        let mut unique_dest = dest_file_path.clone();
                                        loop {
                                            let new_name =
                                                format!("{}_{}.{}", snake_stem, counter, ext_lower);
                                            let candidate = model_subfolder.join(new_name);
                                            if !candidate.exists() {
                                                unique_dest = candidate;
                                                break;
                                            }
                                            if files_are_identical(&file_path, &candidate)
                                                .unwrap_or(false)
                                            {
                                                unique_dest = candidate;
                                                break;
                                            }
                                            counter += 1;
                                        }
                                        fs::copy(&file_path, &unique_dest)?;
                                        let _ = fs::remove_file(&file_path);
                                    }
                                } else {
                                    fs::copy(&file_path, &dest_file_path)?;
                                    let _ = fs::remove_file(&file_path);
                                }

                                if is_model {
                                    moved_models += 1;
                                } else {
                                    moved_textures += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    cleanup_empty_dirs(assets_root, &target_dir)?;

    Ok((moved_models, moved_textures))
}

fn cleanup_empty_dirs(dir: &Path, target_dir: &Path) -> Result<bool> {
    let mut is_empty = true;

    if dir == target_dir {
        return Ok(false);
    }

    if dir.is_dir() {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(false),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if path != target_dir {
                    if !cleanup_empty_dirs(&path, target_dir)? {
                        is_empty = false;
                    }
                } else {
                    is_empty = false;
                }
            } else {
                is_empty = false;
            }
        }

        if is_empty && dir != target_dir {
            let _ = fs::remove_dir(dir);
        }
    }

    Ok(is_empty)
}

/// Scans the assets root directory (strictly ignoring the `3d_models` and `images` target folders)
/// for images and .xcf files, moving them into a root `images` folder while preserving
/// their relative subfolder hierarchy.
pub fn organize_images(assets_root: &PathBuf) -> Result<usize> {
    let target_dir = assets_root.join("images");
    fs::create_dir_all(&target_dir).context(format!(
        "Failed to create root images directory: {:?}",
        target_dir
    ))?;

    let image_extensions = ["png", "jpg", "jpeg", "xcf"];
    let mut moved_count = 0;

    let model_dir = assets_root.join("3d_models");

    let mut file_paths = Vec::new();

    // Walk through assets, skipping 'images' and '3d_models'
    for entry in WalkDir::new(assets_root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| e.path() != target_dir && e.path() != model_dir)
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if image_extensions.contains(&ext_lower.as_str()) {
                    file_paths.push(path.to_path_buf());
                }
            }
        }
    }

    for file_path in file_paths {
        let relative_path = match file_path.strip_prefix(assets_root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Transform components to snake_case for consistency
        let snake_relative_path = transform_path_to_snake_case(relative_path);
        let dest_file_path = target_dir.join(&snake_relative_path);

        // Idempotency check
        if dest_file_path.exists() {
            if files_are_identical(&file_path, &dest_file_path).unwrap_or(false) {
                let _ = fs::remove_file(&file_path);
                continue;
            } else {
                let parent_d = dest_file_path.parent().unwrap_or(&target_dir);
                let stem = dest_file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file");
                let ext = dest_file_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                let mut counter = 1;
                let mut unique_dest = dest_file_path.clone();
                loop {
                    let new_name = if ext.is_empty() {
                        format!("{}_{}", stem, counter)
                    } else {
                        format!("{}_{}.{}", stem, counter, ext)
                    };
                    let candidate = parent_d.join(new_name);
                    if !candidate.exists() {
                        unique_dest = candidate;
                        break;
                    }
                    if files_are_identical(&file_path, &candidate).unwrap_or(false) {
                        unique_dest = candidate;
                        break;
                    }
                    counter += 1;
                }

                if let Some(parent) = unique_dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&file_path, &unique_dest)?;
                let _ = fs::remove_file(&file_path);
            }
        } else {
            if let Some(parent) = dest_file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&file_path, &dest_file_path)?;
            let _ = fs::remove_file(&file_path);
        }

        moved_count += 1;
    }

    cleanup_empty_dirs_for_images(assets_root, &target_dir, &model_dir)?;

    Ok(moved_count)
}

fn cleanup_empty_dirs_for_images(dir: &Path, target_dir: &Path, model_dir: &Path) -> Result<bool> {
    let mut is_empty = true;

    if dir == target_dir || dir == model_dir {
        return Ok(false);
    }

    if dir.is_dir() {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(false),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if path != target_dir && path != model_dir {
                    if !cleanup_empty_dirs_for_images(&path, target_dir, model_dir)? {
                        is_empty = false;
                    }
                } else {
                    is_empty = false;
                }
            } else {
                is_empty = false;
            }
        }

        if is_empty && dir != target_dir && dir != model_dir {
            let _ = fs::remove_dir(dir);
        }
    }

    Ok(is_empty)
}
