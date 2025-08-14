use std::fs::{self};
use std::path::Path;

use crate::encode::encode_frames;
use crate::scenes::{
    FramesDistribution, SceneDetectionMethod, get_scene_file, parse_scene_file,
    write_scene_list_to_file,
};
use crate::ssimulacra2::ssimu2_frames_selected;
use crate::transnetv2::transnet::run_transnetv2;
use crate::vapoursynth::{SourcePlugin, seconds_to_frames};
use crate::vpy_files::create_vpy_file;
use eyre::{OptionExt, Result};

#[allow(clippy::too_many_arguments)]
pub fn run_loop<'a>(
    input: &'a Path,
    scene_boosted: &'a Path,
    av1an_params: &'a str,
    encoder_params: &'a str,
    crf: &[u8],
    target_quality: f64,
    min_target_quality: f64,
    velocity_preset: i32,
    n_frames: Option<u32>,
    s_frames: f64,
    frames_distribution: FramesDistribution,
    scene_detection_method: SceneDetectionMethod,
    filter_frames: bool,
    workers: u32,
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
    extra_split_seconds: i64,
    extra_split_frames: Option<i64>,
    min_scene_len_sec: i64,
    min_scene_len: Option<i64>,
    threshold: f32,
    fade_threshold_low: f32,
    min_fade_len: i64,
    merge_gap: i64,
    enable_fade_detection: bool,
    scene_predictions: bool,
    percentile: u8,
    hardcut_scenes: bool,
) -> Result<&'a Path> {
    println!("\nRunning frame-boost\n");

    let mut scene_list = match scene_detection_method {
        SceneDetectionMethod::Av1an => {
            // Generating original scenes
            let scene_av1an_params = update_chunk_method(av1an_params, importer_scene);
            let scene_av1an_params = if let Some(extra_split_frames) = extra_split_frames {
                update_extra_split(&scene_av1an_params, extra_split_frames)
            } else {
                update_extra_split_sec(&scene_av1an_params, extra_split_seconds)
            };
            let scene_av1an_params = if let Some(min_scene_len) = min_scene_len {
                update_min_scene_len(&scene_av1an_params, min_scene_len)
            } else {
                scene_av1an_params
            };

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
                get_scene_file(vpy_scene_file, temp_folder, &scene_av1an_params, clean)?;
            parse_scene_file(&original_scenes_file)?
        }
        SceneDetectionMethod::TransnetV2 => {
            println!("Obtaining scene using transnetv2-rs\n");
            let (scene_list, hardcut_list) = run_transnetv2(
                input,
                None,
                false,
                *importer_scene,
                temp_folder,
                verbose,
                encoder_params,
                crop,
                downscale,
                detelecine,
                extra_split_seconds,
                extra_split_frames,
                min_scene_len_sec,
                min_scene_len,
                threshold,
                fade_threshold_low,
                min_fade_len,
                merge_gap,
                enable_fade_detection,
                scene_predictions,
            )?;
            println!();
            if hardcut_scenes {
                let output_name = format!(
                    "[HARDCUT-SCENES]_{}.json",
                    input
                        .file_stem()
                        .ok_or_eyre("No file name")?
                        .to_str()
                        .ok_or_eyre("Invalid UTF-8 in input path")?
                );
                let hardcut_path = input.with_file_name(output_name);
                write_scene_list_to_file(hardcut_list, &hardcut_path)?;
            }
            write_scene_list_to_file(scene_list.clone(), &temp_folder.join("scenes.json"))?;
            scene_list
        }
    };

    let first_crf = crf.first().unwrap();
    scene_list.assign_indexes();
    scene_list.update_crf(*first_crf);
    scene_list.with_zone_overrides(av1an_params, encoder_params);

    // New params
    let temp_av1an_params = update_chunk_method(av1an_params, importer_encoding);
    let temp_av1an_params = update_split_method(&temp_av1an_params, "none".to_owned());
    let temp_av1an_params =
        update_extra_split_and_min_scene_len(&temp_av1an_params, Some(0), Some(0), Some(0));
    let temp_av1an_params = update_workers(&temp_av1an_params, workers);
    let temp_encoder_params = remove_crf_param(encoder_params);
    let temp_encoder_params = update_preset(velocity_preset, &temp_encoder_params);

    // crfs
    let crfs = crf.to_vec();
    let iter_crfs: Vec<u8> = crfs[..crfs.len().saturating_sub(1)].to_vec();

    if crfs.len() == 1 {
        scene_list.update_crf(crfs[0]);
        scene_list.print_crf_percentages();
    }

    let mut scene_list_frames = scene_list.clone();
    scene_list_frames.with_zone_overrides(&temp_av1an_params, &temp_encoder_params);

    let n_frames = match n_frames {
        Some(n_frames) => n_frames,
        None => seconds_to_frames(s_frames, input, importer_scene, temp_folder)?,
    };

    scene_list_frames = match frames_distribution {
        FramesDistribution::Center => scene_list_frames.with_center_expanding_frames(n_frames),
        FramesDistribution::Evenly => scene_list_frames.with_evenly_spaced_frames(n_frames),
        FramesDistribution::StartMiddleEnd => scene_list.with_start_middle_end_frames(n_frames),
    };

    for (i, crf) in iter_crfs.iter().enumerate() {
        println!("\nCycle: {i}, CRF: {crf}\n");
        let scenes_path = temp_folder.join(format!("scenes_{crf}.json"));
        let vpy_path = temp_folder.join(format!("vpy_{crf}.vpy"));
        let encode_path = temp_folder.join(format!("encode_{crf}.mkv"));

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
            scene_list_frames.filter_by_frame_score(
                target_quality,
                min_target_quality,
                crfs[i + 1],
                percentile,
            );
        } else {
            scene_list_frames.update_crf(crfs[i + 1]);
        }

        scene_list.sync_crf_by_index(&scene_list_frames);

        println!("\nUpdated data:");
        scene_list.print_updated_data(percentile);
        scene_list.print_stats()?;

        scene_list.print_crf_percentages();

        if clean {
            fs::remove_file(&scenes_path)?;
            fs::remove_file(&vpy_path)?;
            fs::remove_file(&encode_path)?;
        }

        if scene_list_frames.split_scenes.is_empty() {
            break;
        }
    }

    scene_list.write_crf_data(crf_data_file, input, percentile)?;
    scene_list.update_scenes();
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

    if let Some(index) = args.iter().position(|arg| arg == "--preset")
        && index + 1 < args.len() {
            args[index + 1] = velocity_preset.to_string();
        }

    args.join(" ")
}

pub fn update_extra_split_and_min_scene_len(
    params: &str,
    new_extra_split: Option<u32>,
    new_extra_split_sec: Option<u32>,
    new_min_scene_len: Option<u32>,
) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_extra_split = false;
    let mut found_extra_split_sec = false;
    let mut found_min_scene_len = false;

    while let Some(token) = tokens.next() {
        match token {
            "--extra-split" if new_extra_split.is_some() => {
                tokens.next(); // skip old value
                updated_tokens.push("--extra-split".to_string());
                updated_tokens.push(new_extra_split.unwrap().to_string());
                found_extra_split = true;
            }
            "--extra-split-sec" if new_extra_split_sec.is_some() => {
                tokens.next(); // skip old value
                updated_tokens.push("--extra-split-sec".to_string());
                updated_tokens.push(new_extra_split_sec.unwrap().to_string());
                found_extra_split_sec = true;
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

    if !found_extra_split
        && let Some(extra_split) = new_extra_split {
            updated_tokens.push("--extra-split".to_string());
            updated_tokens.push(extra_split.to_string());
        }

    if !found_extra_split_sec
        && let Some(extra_split_sec) = new_extra_split_sec {
            updated_tokens.push("--extra-split-sec".to_string());
            updated_tokens.push(extra_split_sec.to_string());
        }

    if !found_min_scene_len
        && let Some(min_scene_len) = new_min_scene_len {
            updated_tokens.push("--min-scene-len".to_string());
            updated_tokens.push(min_scene_len.to_string());
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

pub fn update_workers(params: &str, new_workers: u32) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_workers = false;

    while let Some(token) = tokens.next() {
        match token {
            "--workers" => {
                tokens.next(); // skip old value
                updated_tokens.push("--workers".to_string());
                updated_tokens.push(new_workers.to_string());
                found_workers = true;
            }
            _ => {
                updated_tokens.push(token.to_string());
            }
        }
    }

    // Append if not found
    if !found_workers {
        updated_tokens.push("--workers".to_string());
        updated_tokens.push(new_workers.to_string());
    }

    updated_tokens.join(" ")
}

pub fn remove_crf_param(params: &str) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();

    while let Some(token) = tokens.next() {
        if token == "--crf" {
            tokens.next(); // Skip the value following --crf
        } else {
            updated_tokens.push(token.to_string());
        }
    }

    updated_tokens.join(" ")
}

/// Extracts the value of a command-line argument from a parameter string
pub fn get_arg_value(params: &str, arg_name: &str) -> Option<String> {
    let mut tokens = params.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        if token == arg_name
            && let Some(value) = tokens.next() {
                return Some(value.to_string());
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

/// Updates or adds the `--extra-split` flag
pub fn update_extra_split(params: &str, new_value: i64) -> String {
    update_flag_with_value(params, "--extra-split", new_value)
}

/// Updates or adds the `--extra-split-sec` flag
pub fn update_extra_split_sec(params: &str, new_value: i64) -> String {
    update_flag_with_value(params, "--extra-split-sec", new_value)
}

/// Updates or adds the `--min-scene-len` flag
pub fn update_min_scene_len(params: &str, new_value: i64) -> String {
    update_flag_with_value(params, "--min-scene-len", new_value)
}

/// Helper function to update or insert a flag and its value
fn update_flag_with_value(params: &str, flag: &str, new_value: i64) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_flag = false;

    while let Some(token) = tokens.next() {
        if token == flag {
            tokens.next(); // Skip old value
            updated_tokens.push(flag.to_string());
            updated_tokens.push(new_value.to_string());
            found_flag = true;
        } else {
            updated_tokens.push(token.to_string());
        }
    }

    if !found_flag {
        updated_tokens.push(flag.to_string());
        updated_tokens.push(new_value.to_string());
    }

    updated_tokens.join(" ")
}
