use std::collections::HashMap;
use std::fs::{self};
use std::path::Path;

use crate::chunk::{Chunk, ChunkList};
use crate::encode::encode_frames;
use crate::math::{self, Score, get_stats};
use crate::scenes::{
    FrameSelection, FramesDistribution, get_scene_file, parse_scene_file, write_scene_list_to_file,
};
use crate::ssimulacra2::ssimu2_frames_selected;
use crate::vapoursynth::SourcePlugin;
use crate::vpy_files::create_vpy_file;
use eyre::{OptionExt, Result};

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
    detelecining: bool,
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
        detelecining,
        encoder_params,
        temp_folder,
        clean,
    )?;
    let original_scenes_file =
        get_scene_file(vpy_scene_file, temp_folder, &temp_av1an_params, clean)?;
    let scene_list = parse_scene_file(&original_scenes_file)?;

    // New params
    let temp_av1an_params = update_chunk_method(av1an_params, importer_encoding);
    let temp_encoder_params = update_preset(velocity_preset, encoder_params);
    let temp_av1an_params = update_split_method(&temp_av1an_params, "none".to_owned());
    let temp_av1an_params =
        update_extra_split_and_min_scene_len(&temp_av1an_params, Some(0), Some(1));

    let last_crf = crf.last().unwrap();

    let chunks: Vec<Chunk> = scene_list
        .scenes
        .iter()
        .map(|scene| Chunk {
            crf: *last_crf,
            scores: vec![Score::default(); n_frames as usize],
            scene: scene.clone(),
        })
        .collect();
    let mut chunk_list = ChunkList {
        chunks,
        frames: scene_list.frames,
    };

    let mut crfs = crf.to_vec();
    let iter_crfs: Vec<u8> = crfs.iter().skip(1).rev().copied().collect();
    crfs.reverse();
    // iter_crfs.insert(0, 0);

    for (i, crf) in iter_crfs.iter().enumerate() {
        println!("\nCycle: {}, CRF: {}\n", i, crf);
        let scenes_path = temp_folder.join(format!("scenes_{}.json", crf));
        let vpy_path = temp_folder.join(format!("vpy_{}.vpy", crf));
        let encode_path = temp_folder.join(format!("encode_{}.mkv", crf));

        // Scenes
        let filtered_scene_list_with_zones = if filter_frames {
            chunk_list.to_scene_list_with_zones_filtered(
                &temp_av1an_params,
                &temp_encoder_params,
                ssimu2_score,
            )
        } else {
            chunk_list.to_scene_list_with_zones(&temp_av1an_params, &temp_encoder_params)
        };

        let scene_list_selected_frames =
            filtered_scene_list_with_zones.as_selected_frames(n_frames);
        let scenes_file_selected_frames =
            write_scene_list_to_file(&scene_list_selected_frames, &scenes_path)?;

        let frame_selection = FrameSelection {
            scene_list: filtered_scene_list_with_zones.clone(),
            n_frames,
            distribution: frames_distribution,
        };

        // Temp encode
        let vpy_file = create_vpy_file(
            input,
            &vpy_path,
            Some(&frame_selection),
            importer_encoding,
            crop,
            downscale,
            detelecining,
            encoder_params,
            temp_folder,
            clean,
        )?;
        let encode = encode_frames(
            vpy_file,
            scenes_file_selected_frames,
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
        let score_list = ssimu2_frames_selected(
            input,
            encode,
            &filtered_scene_list_with_zones,
            n_frames,
            frames_distribution,
            importer_metrics,
            temp_folder,
            verbose,
        )?;

        if *crf == *last_crf {
            for (chunk, new_scores) in chunk_list
                .chunks
                .iter_mut()
                .zip(score_list.scores.chunks(n_frames.try_into().unwrap()))
            {
                chunk.scores = new_scores.to_vec()
            }
        }

        for new_scores in score_list.scores.chunks(n_frames.try_into().unwrap()) {
            if let Some(chunk) = chunk_list.chunks.iter_mut().find(|chunk| {
                chunk.scores.first().unwrap().frame == new_scores.first().unwrap().frame
            }) {
                chunk.scores = new_scores.to_vec();
                let mean = math::mean(&chunk.clone().to_score_list());
                // if chunk.scores.iter().any(|score| score.value < ssimu2_score) {
                //     chunk.crf = crfs[i + 1]
                // }
                if mean < ssimu2_score {
                    chunk.crf = crfs[i + 1]
                }
            }
        }
        // for new_score in score_list.scores {
        //     if let Some(values) = chunk_list
        //         .chunks
        //         .iter_mut()
        //         .find(|v| v.scores.frame == new_score.frame)
        //     {
        //         values.score = *new_score;
        //         values.crf = match new_score.value {
        //             x if x <= ssimu2_score => crfs[i],
        //             _ => values.crf,
        //         };
        //     }
        // }

        if verbose {
            println!("\nUpdated data:\n");
            for (i, chunk) in chunk_list.chunks.iter().enumerate() {
                let score_list = chunk.clone().to_score_list();
                let mean_score = math::mean(&score_list);
                // let score_min = score_min.scores.first().unwrap();

                // let score_max = math::max(&score_list)?;
                // let score_max = score_max.scores.first().unwrap();

                println!(
                    "scene: {:4}, crf: {:3}, frame-range: {:6} {:6}, mean-score: {:6.2}",
                    i,
                    chunk.crf,
                    chunk.scene.start_frame,
                    chunk.scene.end_frame,
                    // score_min.frame,
                    mean_score,
                );
            }
        }

        let percentages = calculate_crf_percentages(&chunk_list);
        let line = percentages
            .iter()
            .map(|(crf, pct)| format!("\nCRF {}: {:.2}%", crf, pct))
            .collect::<Vec<String>>()
            .join(", ");
        println!("{}", line);

        let score_list = &chunk_list.to_score_list();
        let stats = get_stats(score_list)?;
        println!("\n{}", stats);

        if clean {
            fs::remove_file(&scenes_path)?;
            fs::remove_file(&vpy_path)?;
            fs::remove_file(&encode_path)?;
        }

        if score_list.scores.iter().all(|x| x.value >= ssimu2_score) {
            write_crf_data(crf_data_file, input, &chunk_list)?;
            break;
        }

        if iter_crfs.last() == Some(crf) {
            write_crf_data(crf_data_file, input, &chunk_list)?;
        }
    }

    let scene_list_with_zones = chunk_list.to_scene_list_with_zones(av1an_params, encoder_params);

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

pub fn write_crf_data(
    crf_data_file: Option<&Path>,
    input: &std::path::Path,
    chunk_list: &ChunkList,
) -> Result<()> {
    if let Some(crf_data_file) = crf_data_file {
        // Build the entire output string first
        let mut output = String::new();

        output.push_str("[INFO]\n");

        // Add filename header
        let video_name = input
            .file_name()
            .ok_or_eyre("Error getting file name")?
            .to_str()
            .ok_or_eyre("Invalid UTF-8")?;
        let filename = format!("Video: {}\n", video_name);
        output.push_str(&filename);

        // Add CRF percentages
        let percentages = calculate_crf_percentages(chunk_list);
        let percentages_line = percentages
            .iter()
            .map(|(crf, pct)| format!("CRF {}: {:.2}%", crf, pct))
            .collect::<Vec<String>>()
            .join(", ");
        output.push_str("Distribution: ");
        output.push_str(&percentages_line);
        output.push_str("\n\n");

        output.push_str("[DATA]\n");
        // Add chunk details
        for (i, chunk) in chunk_list.chunks.iter().enumerate() {
            let score_list = chunk.clone().to_score_list();
            let mean_score = math::mean(&score_list);
            // let score_min = score_min.scores.first().unwrap();
            // let score_max = math::max(&score_list)?;
            // let score_max = score_max.scores.first().unwrap();

            output.push_str(&format!(
                "scene: {:4}, crf: {:3}, frame-range: {:6} {:6}, mean-score: {:6.2}\n",
                i,
                chunk.crf,
                chunk.scene.start_frame,
                chunk.scene.end_frame,
                // score_max.frame,
                // score_max.value,
                mean_score,
            ));
        }

        // Write everything at once
        std::fs::write(crf_data_file, &output)?;

        println!(
            "CRF data successfully written to {}",
            crf_data_file
                .as_os_str()
                .to_str()
                .ok_or_eyre("Invalid UTF-8")?
        );
    }

    Ok(())
}

pub fn calculate_crf_percentages(chunk_list: &ChunkList) -> Vec<(u8, f64)> {
    let crf_values: Vec<u8> = chunk_list.chunks.iter().map(|chunk| chunk.crf).collect();
    let crf_vec: Vec<u8> = crf_values.into_iter().collect();
    let total = crf_vec.len() as f64;

    let mut counts = crf_vec.iter().fold(HashMap::new(), |mut acc, &val| {
        *acc.entry(val).or_insert(0) += 1;
        acc
    });

    let mut percentages: Vec<(u8, f64)> = counts
        .drain()
        .map(|(val, count)| (val, (count as f64 / total) * 100.0))
        .collect();

    percentages.sort_by_key(|&(val, _)| val);
    percentages
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
