use std::fs::{self};
use std::path::Path;

use crate::encode::encode_frames;
use crate::scenes::{
    FramesDistribution, get_scene_file, parse_scene_file, write_scene_list_to_file,
};
use crate::ssimulacra2::ssimu2_frames_selected;
use crate::vapoursynth::SourcePlugin;
use crate::vpy_files::create_vpy_file;
use eyre::Result;

#[allow(clippy::too_many_arguments)]
pub fn run_loop<'a>(
    input: &'a Path,
    scene_boosted: &'a Path,
    av1an_params: &'a str,
    encoder_params: &'a str,
    crf: &[u8],
    ssimu2_score: f64,
    velocity_preset: i32,
    n_frames: u32,
    frames_distribution: FramesDistribution,
    filter_frames: bool,
    importer_metrics: &SourcePlugin,
    importer_encoding: &SourcePlugin,
    importer_scene: &SourcePlugin,
    crf_data_file: Option<&'a Path>,
    crop: Option<&str>,
    downscale: bool,
    detelecine: bool,
    clean: bool,
    verbose: bool,
    temp_folder: &'a Path,
) -> Result<&'a Path> {
    println!("\nRunning frame-boost\n");

    // Generating original scenes
    let temp_av1an_params = update_chunk_method(av1an_params, importer_scene);
    let vpy_scene_path = temp_folder.join("scene.vpy");

    let vpy_scene_file = create_vpy_file(
        input,
        &vpy_scene_path,
        None,
        importer_scene,
        crop,
        downscale,
        detelecine,
        encoder_params,
        temp_folder,
        clean,
    )?;
    let original_scenes_file =
        get_scene_file(vpy_scene_file, temp_folder, &temp_av1an_params, clean)?;
    let mut scene_list = parse_scene_file(&original_scenes_file)?;

    let first_crf = crf.first().unwrap();
    scene_list.assign_indexes();
    scene_list.update_crf(*first_crf);
    scene_list.with_zone_overrides(av1an_params, encoder_params);

    // New params
    let temp_av1an_params = update_chunk_method(av1an_params, importer_encoding);
    let temp_encoder_params = update_preset(velocity_preset, encoder_params);
    let temp_av1an_params = update_split_method(&temp_av1an_params, "none".to_owned());
    let temp_av1an_params =
        update_extra_split_and_min_scene_len(&temp_av1an_params, Some(0), Some(0));

    // crfs
    let crfs = crf.to_vec();
    let iter_crfs: Vec<u8> = crfs[..crfs.len().saturating_sub(1)].to_vec();

    if crfs.len() == 1 {
        scene_list.update_crf(crfs[0]);
        scene_list.print_crf_percentages();
    }

    let mut scene_list_frames = scene_list.clone();
    scene_list_frames = match frames_distribution {
        FramesDistribution::Center => scene_list_frames.with_center_expanding_frames(n_frames),
        FramesDistribution::Evenly => scene_list_frames.with_evenly_spaced_frames(n_frames),
    };

    for (i, crf) in iter_crfs.iter().enumerate() {
        println!("\nCycle: {}, CRF: {}\n", i, crf);
        let scenes_path = temp_folder.join(format!("scenes_{}.json", crf));
        let vpy_path = temp_folder.join(format!("vpy_{}.vpy", crf));
        let encode_path = temp_folder.join(format!("encode_{}.mkv", crf));

        scene_list_frames = scene_list_frames.with_contiguous_frames();
        let filter_scene_file = write_scene_list_to_file(scene_list_frames.clone(), &scenes_path)?;

        // Temp encode
        let vpy_file = create_vpy_file(
            input,
            &vpy_path,
            Some(&scene_list_frames),
            importer_encoding,
            crop,
            downscale,
            detelecine,
            encoder_params,
            temp_folder,
            clean,
        )?;
        let encode = encode_frames(
            vpy_file,
            filter_scene_file,
            &encode_path,
            &temp_av1an_params,
            &temp_encoder_params,
            clean,
            temp_folder,
        )?;

        // Scores
        if verbose {
            println!("\nGet simulacra scores\n")
        }
        ssimu2_frames_selected(
            input,
            encode,
            &mut scene_list_frames,
            // n_frames,
            // frames_distribution,
            importer_metrics,
            temp_folder,
            verbose,
            encoder_params,
            crop,
            downscale,
            detelecine,
        )?;

        scene_list.sync_scores_by_index(&scene_list_frames);

        if filter_frames {
            scene_list_frames.filter_by_frame_score(ssimu2_score, crfs[i + 1]);
        } else {
            scene_list_frames.update_crf(crfs[i + 1]);
        }
        
        scene_list.sync_crf_by_index(&scene_list_frames);

        if verbose {
            println!("\nUpdated data:");
            scene_list.print_updated_data();
            scene_list.print_stats()?;
        }

        scene_list.print_crf_percentages();

        if clean {
            fs::remove_file(&scenes_path)?;
            fs::remove_file(&vpy_path)?;
            fs::remove_file(&encode_path)?;
        }

        if scene_list_frames.scenes.is_empty() {
            break;
        }
    }

    scene_list.write_crf_data(crf_data_file, input)?;
    write_scene_list_to_file(scene_list, scene_boosted)?;

    if clean && temp_folder.exists() {
        fs::remove_dir_all(temp_folder)?;
    }

    Ok(scene_boosted)
}

#[derive(Debug)]
pub struct CrfRange {
    pub min: u32,
    pub max: u32,
}

pub fn parse_crf_and_strip(params: &str) -> (Option<CrfRange>, String) {
    let mut tokens = params.split_whitespace().peekable();
    let mut new_params = Vec::new();
    let mut crf: Option<CrfRange> = None;

    while let Some(token) = tokens.next() {
        if token == "--crf" {
            if let Some(value) = tokens.next() {
                if let Some((min_str, max_str)) = value.split_once('~') {
                    if let (Ok(min), Ok(max)) = (min_str.parse(), max_str.parse()) {
                        crf = Some(CrfRange { min, max });
                    }
                } else if let Ok(single) = value.parse() {
                    crf = Some(CrfRange {
                        min: single,
                        max: single,
                    });
                }
            }
        } else {
            new_params.push(token.to_string());
        }
    }

    (crf, new_params.join(" "))
}

pub fn update_preset(velocity_preset: i32, encoder_params: &str) -> String {
    let mut args: Vec<String> = encoder_params
        .split_whitespace()
        .map(String::from)
        .collect();

    if let Some(index) = args.iter().position(|arg| arg == "--preset") {
        if index + 1 < args.len() {
            args[index + 1] = velocity_preset.to_string();
        }
    }

    args.join(" ")
}

pub fn update_extra_split_and_min_scene_len(
    params: &str,
    new_extra_split: Option<u32>,
    new_min_scene_len: Option<u32>,
) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_extra_split = false;
    let mut found_min_scene_len = false;

    while let Some(token) = tokens.next() {
        match token {
            "--extra-split" if new_extra_split.is_some() => {
                tokens.next(); // skip old value
                updated_tokens.push("--extra-split".to_string());
                updated_tokens.push(new_extra_split.unwrap().to_string());
                found_extra_split = true;
            }
            "--min-scene-len" if new_min_scene_len.is_some() => {
                tokens.next(); // skip old value
                updated_tokens.push("--min-scene-len".to_string());
                updated_tokens.push(new_min_scene_len.unwrap().to_string());
                found_min_scene_len = true;
            }
            _ => {
                updated_tokens.push(token.to_string());
            }
        }
    }

    if !found_extra_split {
        if let Some(extra_split) = new_extra_split {
            updated_tokens.push("--extra-split".to_string());
            updated_tokens.push(extra_split.to_string());
        }
    }

    if !found_min_scene_len {
        if let Some(min_scene_len) = new_min_scene_len {
            updated_tokens.push("--min-scene-len".to_string());
            updated_tokens.push(min_scene_len.to_string());
        }
    }

    updated_tokens.join(" ")
}

pub fn update_split_method(params: &str, new_split_method: String) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_split_method = false;

    while let Some(token) = tokens.next() {
        match token {
            "--split-method" => {
                tokens.next(); // skip old value
                updated_tokens.push("--split-method".to_string());
                updated_tokens.push(new_split_method.to_string());
                found_split_method = true;
            }
            _ => {
                updated_tokens.push(token.to_string());
            }
        }
    }

    // Append if not found
    if !found_split_method {
        updated_tokens.push("--split-method".to_string());
        updated_tokens.push(new_split_method.to_string());
    }

    updated_tokens.join(" ")
}

/// Extracts the value of a command-line argument from a parameter string
pub fn get_arg_value(params: &str, arg_name: &str) -> Option<String> {
    let mut tokens = params.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        if token == arg_name {
            if let Some(value) = tokens.next() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Checks the chunk method in the params and returns the corresponding ImporterPlugin
pub fn check_chunk_method(params: &str) -> Option<SourcePlugin> {
    let chunk_method = get_arg_value(params, "--chunk-method")?;

    match chunk_method.as_str() {
        "lsmash" => Some(SourcePlugin::Lsmash),
        "bestsource" => Some(SourcePlugin::Bestsource),
        _ => None,
    }
}

pub fn update_chunk_method(params: &str, new_chunk_method: &SourcePlugin) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_chunk_method = false;

    while let Some(token) = tokens.next() {
        match token {
            "--chunk-method" | "-m" => {
                tokens.next(); // skip old value
                updated_tokens.push("--chunk-method".to_string());
                updated_tokens.push(new_chunk_method.as_str().to_string());
                found_chunk_method = true;
            }
            _ => {
                updated_tokens.push(token.to_string());
            }
        }
    }

    // Append if not found
    if !found_chunk_method {
        updated_tokens.push("--chunk-method".to_string());
        updated_tokens.push(new_chunk_method.as_str().to_string());
    }

    updated_tokens.join(" ")
}
