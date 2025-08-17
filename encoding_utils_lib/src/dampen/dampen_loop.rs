use std::fs::{self};
use std::path::{Path, PathBuf};

use crate::dampen::chunks::ChunkList;
use crate::dampen::done::Done;
use crate::encode::resume_encode;
use crate::scenes::SceneList;
use bytesize::ByteSize;
use eyre::{Context, OptionExt, Result};
use fs_extra::dir::{CopyOptions, copy};
use fs_extra::file::{CopyOptions as FileCopyOptions, copy as copy_file};
use itertools::Itertools;

#[allow(clippy::too_many_arguments)]
pub fn dampen_loop<'a>(
    input: &'a Path,
    output: &'a Path,
    scene_boosted: &'a Path,
    scene_dampened: &'a Path,
    av1an_params: &'a str,
    crfs: &[u8],
    size_threshold: ByteSize,
    crf_data_file: Option<&'a Path>,
    temp_folder: &'a Path,
) -> Result<&'a Path> {
    println!("\nRunning size-dampener\n");
    println!("Size Threshold: {}", size_threshold);

    // Initialize scene data
    let mut scene_list = SceneList::parse_scene_file(scene_boosted)?;
    scene_list.assign_indexes();
    scene_list.sync_crf_from_zone_overrides()?;

    // Setup paths
    let done_path = temp_folder.join("done.json");
    let chunks_path = temp_folder.join("chunks.json");
    let encode_scenes_path = temp_folder.join("encode");

    // BackUp paths
    let done_backup = temp_folder.join("done_backup.json");
    let chunks_backup = temp_folder.join("chunks_backup.json");
    let encode_backup = temp_folder.join("encode_backup");

    // 1. Backup JSON files
    if done_path.exists() {
        copy_file(
            &done_path,
            &done_backup,
            &FileCopyOptions::new().overwrite(true),
        )
        .wrap_err("Failed to backup done.json")?;
    }

    if chunks_path.exists() {
        copy_file(
            &chunks_path,
            &chunks_backup,
            &FileCopyOptions::new().overwrite(true),
        )
        .wrap_err("Failed to backup chunks.json")?;
    }

    // 2. Backup encode directory (with all contents)
    if encode_scenes_path.exists() {
        // Create the backup directory first
        std::fs::create_dir_all(&encode_backup)
            .wrap_err("Failed to create encode_backup directory")?;

        let options = CopyOptions::new()
            .overwrite(true) // Overwrite existing files
            .content_only(true) // Copy the directory itself
            .copy_inside(true); // Don't copy inside existing directory

        copy(&encode_scenes_path, &encode_backup, &options)
            .wrap_err("Failed to backup encode directory")?;
    }

    // Load state files
    let mut done = Done::parse_done_file(&done_path)?;
    let mut chunk_list = ChunkList::parse_chunks_file(&chunks_path)?;

    // Process CRF values
    let max_crf = *crfs.iter().max().ok_or_eyre("Empty CRF list provided")?;
    let crfs = crfs.iter().sorted().copied().collect::<Vec<u8>>();

    // Initialize scene size tracking
    let mut scene_size_list = SceneSizeList::new(
        encode_scenes_path,
        &scene_list,
        size_threshold,
        max_crf,
        crfs,
    )?;

    // Early exit if all scenes meet threshold
    if !scene_size_list.is_not_ready() {
        println!("ALL SCENES BELOW THE SIZE THRESHOLD");
        return Ok(scene_dampened);
    }

    // Main processing loop
    let mut iteration = 0;
    while scene_size_list.is_not_ready() {
        println!("\n\n=== Iteration {} ===", iteration);
        // println!("{scene_size_list:#?}");
        scene_size_list.print_not_ready();

        // Update state files
        done.update_from_scene_sizes(&scene_size_list)?;
        // println!("{done:#?}");
        chunk_list.update_crf_from_scene_sizes(&scene_size_list)?;

        done.write_done_to_file(&done_path)?;
        chunk_list.write_chunks_to_file(&chunks_path)?;

        // Run encode
        let encode_path = temp_folder.join(format!("encode_size_dampener_{}.mkv", iteration));
        resume_encode(
            input,
            scene_boosted,
            &encode_path,
            av1an_params,
            &format!("SIZE DAMPENER ITERATION {}", iteration),
            false,
            temp_folder,
        )?;

        // Reload state to ensure consistency
        done = Done::parse_done_file(&done_path)?;
        chunk_list = ChunkList::parse_chunks_file(&chunks_path)?;

        // Cleanup and update for next iteration
        fs::remove_file(&encode_path)?;
        scene_size_list.update_sizes()?;

        match iteration {
            0 => scene_size_list.initial_update_crfs(),
            _ => scene_size_list.update_crfs(),
        }

        iteration += 1;
    }

    // Final encode
    resume_encode(
        input,
        scene_boosted,
        output,
        av1an_params,
        "FINAL ENCODE - SIZE DAMPENER",
        false,
        temp_folder,
    )?;

    // Final status report
    scene_size_list.update_sizes()?;
    scene_size_list.update_crfs();
    scene_size_list.print_updated_scenes();

    // Output new scene.json file
    scene_list.update_crfs_from_sizes(&scene_size_list)?;
    scene_list.update_scenes();
    scene_list.write_scene_list_to_file(scene_dampened)?;
    scene_list.write_crf_data(crf_data_file, input, None, false)?;

    Ok(scene_dampened)
}

#[derive(Debug, Default, Clone)]
pub struct SceneSize {
    pub index: u32,
    pub original_size: ByteSize,
    pub new_size: ByteSize,
    pub original_crf: u8,
    pub new_crf: u8,
    pub ready: bool,
}

#[derive(Debug, Default, Clone)]
pub struct SceneSizeList {
    pub scenes_path: PathBuf,
    pub scenes: Vec<SceneSize>,
    pub size_threshold: ByteSize,
    pub max_crf: u8,
    pub crfs: Vec<u8>,
}

impl SceneSizeList {
    pub fn new(
        scenes_path: PathBuf,
        scene_list: &SceneList,
        size_threshold: ByteSize,
        max_crf: u8,
        crfs: Vec<u8>,
    ) -> eyre::Result<SceneSizeList> {
        let mut result = Vec::new();

        for entry in fs::read_dir(&scenes_path)? {
            let entry = entry?;
            let path = entry.path();
            // Skip directories and non-IVF files
            if !path.is_file() {
                continue;
            }

            if path.extension().and_then(|e| e.to_str()) != Some("ivf") {
                continue;
            }

            // Get file metadata and size
            let metadata = path.metadata()?;
            let size_u64: u64 = metadata.len();
            let original_size = bytesize::ByteSize(size_u64);

            let file_name = path
                .file_stem() // This gets the filename without extension
                .ok_or_eyre("Error obtaining file name")?
                .to_str()
                .ok_or_eyre("Error converting file name to str")?
                .to_string();
            let index: u32 = file_name.parse()?;
            let original_crf = scene_list
                .split_scenes
                .iter()
                .find(|scene| scene.index == index)
                .ok_or_eyre("Scene not found")?
                .crf;
            // println!("Size: {size}");
            // println!("Size Threshold: {size_threshold}");

            let ready = original_size <= size_threshold || original_crf >= max_crf;

            // let new_crf = crfs
            //     .iter()
            //     .find(|&&crf| crf > original_crf)
            //     .copied()
            //     .unwrap_or(max_crf); // Fallback to max_crf if no larger CRF found

            let new_crf = if ready { original_crf } else { max_crf };

            let scene_size = SceneSize {
                index,
                original_size,
                new_size: original_size,
                original_crf,
                new_crf,
                ready,
            };
            result.push(scene_size);
        }

        Ok(SceneSizeList {
            scenes: result,
            scenes_path,
            size_threshold,
            max_crf,
            crfs,
        })
    }

    pub fn update_sizes(&mut self) -> eyre::Result<()> {
        for entry in fs::read_dir(&self.scenes_path)? {
            let entry = entry?;
            let path = entry.path();

            // Skip directories and non-IVF files
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("ivf") {
                continue;
            }

            let file_name = path
                .file_stem() // This gets the filename without extension
                .ok_or_eyre("Error obtaining file name")?
                .to_str()
                .ok_or_eyre("Error converting file name to str")?
                .to_string();
            let index: u32 = file_name.parse()?;

            // Get file metadata and size
            let metadata = path.metadata()?;
            let size_u64: u64 = metadata.len();
            let size = bytesize::ByteSize(size_u64);

            // Find matching scene and update its size
            if let Some(scene) = self.scenes.iter_mut().find(|s| s.index == index)
                && !scene.ready
            {
                scene.new_size = size;
            }
        }

        Ok(())
    }

    pub fn is_not_ready(&self) -> bool {
        self.scenes.iter().any(|scene| !scene.ready)
    }

    pub fn update_crfs(&mut self) {
        for scene in &mut self.scenes {
            // Skip scenes that are already ready
            if scene.ready {
                continue;
            }

            // If current size is still over threshold, try a higher CRF
            if scene.new_size > self.size_threshold {
                // Find the next higher CRF in the list
                if let Some(higher_crf) =
                    self.crfs.iter().find(|&&crf| crf > scene.new_crf).copied()
                {
                    scene.new_crf = higher_crf;
                } else {
                    // No higher CRF available, mark as ready with max_crf
                    scene.new_crf = self.max_crf;
                    scene.ready = true;
                }
            } else {
                // Size is under threshold, mark as ready
                scene.ready = true;
            }
        }
    }

    /// Special initial CRF update that:
    /// 1. Scenes over threshold after max_crf are marked ready (can't do better)
    /// 2. Scenes under threshold get next CRF after original_crf (starting iteration)
    pub fn initial_update_crfs(&mut self) {
        for scene in &mut self.scenes {
            if scene.ready {
                continue;
            }

            if scene.new_size > self.size_threshold {
                // Already using max_crf and still over threshold - mark ready
                scene.ready = true;
            } else {
                // Under threshold - start iteration from next CRF after original
                if let Some(next_crf) = self
                    .crfs
                    .iter()
                    .find(|&&crf| crf > scene.original_crf)
                    .copied()
                {
                    scene.new_crf = next_crf;
                } else {
                    // No higher CRF available - keep max_crf and mark ready
                    scene.ready = true;
                }
            }
        }
    }

    /// Prints information about scenes that aren't yet ready
    pub fn print_not_ready(&self) {
        println!("\n\nUpdating Scenes:");
        println!("-----------------");

        // Create a sorted vector of scenes
        let mut sorted_scenes = self.scenes.clone();
        sorted_scenes.sort_by_key(|s| s.index);

        for scene in sorted_scenes {
            if !scene.ready {
                println!(
                    "scene: {:4}, original_crf: {}, new_crf: {}, original_size: {}, new_size: {}",
                    scene.index,
                    scene.original_crf,
                    scene.new_crf,
                    scene.original_size,
                    scene.new_size
                );
            }
        }

        println!("-----------------\n");
    }

    /// Prints scenes that changed after updates
    pub fn print_updated_scenes(&self) {
        println!("\n\nFinal - Updated Scenes:");
        println!("-----------------");

        // Create a sorted vector of scenes
        let mut sorted_scenes = self.scenes.clone();
        sorted_scenes.sort_by_key(|s| s.index);

        for scene in &sorted_scenes {
            // Only show scenes where either:
            // 1. The size changed (new_size != original_size), or
            // 2. The CRF changed (new_crf != original_crf)
            if scene.new_size != scene.original_size || scene.new_crf != scene.original_crf {
                println!(
                    "scene: {}, original_crf: {} → new_crf: {}, original_size: {} → new_size: {}",
                    scene.index,
                    scene.original_crf,
                    scene.new_crf,
                    scene.original_size,
                    scene.new_size,
                );
            }
        }

        println!("-----------------\n");
    }
}
