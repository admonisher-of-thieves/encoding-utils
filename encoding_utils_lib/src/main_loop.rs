use std::fs;
use std::path::Path;

use crate::chunk::{Chunk, ChunkList};
use crate::encode::encode_frames;
use crate::math::{Score, get_stats};
use crate::scenes::{get_scene_file, parse_scene_file, write_scene_list_to_file};
use crate::ssimulacra2::ssimu2_frames_scenes;
use crate::vapoursynth::ImporterPlugin;
use crate::vpy_files::create_frames_vpy_file;
use eyre::{OptionExt, Result};

#[allow(clippy::too_many_arguments)]
pub fn run_loop<'a>(
    input: &'a Path,
    scene_boosted: &'a Path,
    av1an_params: &'a str,
    encoder_params: &'a str,
    ssimu2_score: f64,
    velocity_preset: i32,
    step: usize,
    clean: bool,
    verbose: bool,
    temp_folder: &'a Path,
) -> Result<&'a Path> {
    println!("\nRunning scene-boost\n");

    // Generating original scenes
    let original_scenes_path = temp_folder.join("scenes.json");
    let original_scenes_file = get_scene_file(input, &original_scenes_path, av1an_params, clean)?;
    let scene_list = parse_scene_file(original_scenes_file)?;

    // Creating crf list
    let (crf, encoder_params) = parse_crf_and_strip(encoder_params);
    let crf = crf.ok_or_eyre("CRF not found")?;

    let chunks: Vec<Chunk> = scene_list
        .scenes
        .iter()
        .map(|scene| Chunk {
            crf: crf.max,
            score: Score::default(),
            scene: scene.clone(),
        })
        .collect();
    let mut chunk_list = ChunkList {
        chunks,
        frames: scene_list.frames,
    };

    let temp_encoder_params = update_preset(velocity_preset, &encoder_params);

    let mut crfs: Vec<u32> = (crf.min + 1..=crf.max).rev().step_by(step).collect();
    crfs.reverse();
    crfs.push(0);
    crfs.reverse();

    for (i, i_crf) in crfs.iter().enumerate() {
        println!("\nCycle: {}, CRF: {}\n", i, i_crf);
        let scenes_path = temp_folder.join(format!("scenes_{}.json", i_crf));
        let vpy_path = temp_folder.join(format!("vpy_{}.vpy", i_crf));
        let encode_path = temp_folder.join(format!("encode_{}.mkv", i_crf));

        // Scenes
        let mut filtered_scene_list_with_zones = chunk_list.to_scene_list_with_zones_filtered(
            av1an_params,
            &temp_encoder_params,
            ssimu2_score,
            verbose,
        );
        filtered_scene_list_with_zones.update_preset(velocity_preset);
        let scene_list_middle_frames = filtered_scene_list_with_zones.as_middle_frames();
        let scenes_file_middle_frames =
            write_scene_list_to_file(&scene_list_middle_frames, &scenes_path)?;

        // Temp encode
        let vpy_file =
            create_frames_vpy_file(input, &vpy_path, &filtered_scene_list_with_zones, clean)?;
        let new_av1an_params = update_split_method(av1an_params, "none".to_owned());
        let new_av1an_params = update_extra_split_and_min_scene_len(&new_av1an_params, 0, 1);
        let encode = encode_frames(
            vpy_file,
            scenes_file_middle_frames,
            &encode_path,
            &new_av1an_params,
            &temp_encoder_params,
            clean,
        )?;

        // Scores
        let score_list = ssimu2_frames_scenes(
            input,
            encode,
            &filtered_scene_list_with_zones,
            check_chunk_method(av1an_params)
                .ok_or_eyre("--chuck-method not found in av1an_params")?,
            verbose,
        )?;

        if verbose {
            let stats = get_stats(&score_list)?;
            println!("{}", stats)
        }

        if *i_crf == 0 {
            for (chunk, score) in chunk_list.chunks.iter_mut().zip(&score_list.scores) {
                chunk.score = *score
            }
        } else {
            for new_score in &score_list.scores {
                if let Some(values) = chunk_list
                    .chunks
                    .iter_mut()
                    .find(|v| v.score.frame == new_score.frame)
                {
                    values.score = *new_score;
                    values.crf = match new_score.value {
                        x if x <= ssimu2_score => values.crf.saturating_sub(step.try_into()?),
                        _ => values.crf,
                    };
                }
            }
        }

        if clean {
            fs::remove_file(&scenes_path)?;
            fs::remove_file(&vpy_path)?;
            fs::remove_file(&encode_path)?;
        }

        if score_list.scores.iter().all(|x| x.value >= ssimu2_score) {
            break;
        }
    }

    let scene_list_with_zones = chunk_list.to_scene_list_with_zones(av1an_params, &encoder_params);
    write_scene_list_to_file(&scene_list_with_zones, scene_boosted)?;

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
    new_extra_split: u32,
    new_min_scene_len: u32,
) -> String {
    let mut tokens = params.split_whitespace().peekable();
    let mut updated_tokens: Vec<String> = Vec::new();
    let mut found_extra_split = false;
    let mut found_min_scene_len = false;

    while let Some(token) = tokens.next() {
        match token {
            "--extra-split" => {
                tokens.next(); // skip old value
                updated_tokens.push("--extra-split".to_string());
                updated_tokens.push(new_extra_split.to_string());
                found_extra_split = true;
            }
            "--min-scene-len" => {
                tokens.next(); // skip old value
                updated_tokens.push("--min-scene-len".to_string());
                updated_tokens.push(new_min_scene_len.to_string());
                found_min_scene_len = true;
            }
            _ => {
                updated_tokens.push(token.to_string());
            }
        }
    }

    // Append if not found
    if !found_extra_split {
        updated_tokens.push("--extra-split".to_string());
        updated_tokens.push(new_extra_split.to_string());
    }

    if !found_min_scene_len {
        updated_tokens.push("--min-scene-len".to_string());
        updated_tokens.push(new_min_scene_len.to_string());
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
fn get_arg_value(params: &str, arg_name: &str) -> Option<String> {
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
pub fn check_chunk_method(params: &str) -> Option<ImporterPlugin> {
    let chunk_method = get_arg_value(params, "--chunk-method")?;

    match chunk_method.as_str() {
        "lsmash" => Some(ImporterPlugin::Lsmash),
        "bestsource" => Some(ImporterPlugin::Bestsource),
        _ => None,
    }
}
