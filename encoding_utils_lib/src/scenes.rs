use std::{
    collections::HashMap,
    fs::{self},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use clap::ValueEnum;
use eyre::{Ok, OptionExt, Result};

pub fn get_scene_file<'a>(
    scene_vpy_file: &'a Path,
    temp_folder: &'a Path,
    av1an_params: &str,
    clean: bool,
) -> Result<PathBuf> {
    let scenes_path = temp_folder.join("scenes.json");
    if clean && scenes_path.exists() {
        fs::remove_file(&scenes_path)?;
    }
    let vpy_str = scene_vpy_file
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    let mut scene_temp_folder = temp_folder.to_owned();
    scene_temp_folder.push("scene");
    // create_dir_all(&scene_temp_folder)?;
    let scene_temp_folder = scene_temp_folder
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;

    // let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let binding = scenes_path.clone();
    let scene_str = binding.to_str().ok_or_eyre("Invalid UTF-8 in scene path")?;

    println!("Obtaining scene file:\n");

    let av1an_params: Vec<String> = av1an_params
        .split_whitespace()
        .map(str::to_string)
        .collect();

    let mut args: Vec<String> = Vec::from([
        "-i".to_owned(),
        vpy_str.to_owned(),
        "--scenes".to_owned(),
        scene_str.to_owned(),
        "--sc-only".to_owned(),
        "--temp".to_owned(),
        scene_temp_folder.to_owned(),
    ]);

    if !clean {
        args.push("--keep".to_owned());
    }

    args.extend(av1an_params);

    println!("{}", args.join(" "));
    println!();

    Command::new("av1an")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    Ok(scenes_path)
}

pub fn get_scene_file_with_zones<'a>(
    input: &'a Path,
    scenes_zones_path: &'a Path,
    zones_path: &'a Path,
    av1an_params: &str,
    encoder_params: &str,
    override_file: bool,
) -> Result<&'a Path> {
    if override_file && scenes_zones_path.exists() {
        fs::remove_file(scenes_zones_path)?;
    }

    let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let scenes_zones_str = scenes_zones_path
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in scenes path")?;
    let zones_str = zones_path
        .to_str()
        .ok_or_eyre("Invalid UTF-8 in zones path")?;

    println!("Obtaining scene file:\n");

    let av1an_params: Vec<&str> = av1an_params.split_whitespace().collect();
    let construct_params: Vec<&str> = Vec::from([
        "--sc-only",
        "-i",
        input_str,
        "--video-params",
        encoder_params,
        "-y",
        "--scenes",
        scenes_zones_str,
        "--zones",
        zones_str,
    ]);

    let mut args = Vec::new();
    args.extend(av1an_params);
    args.extend(construct_params);

    println!("{}", args.join(" "));

    Command::new("av1an")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    println!("Scene file obtained\n");
    Ok(scenes_zones_path)
}

use serde::{Deserialize, Serialize};

use crate::{
    chapters::ZoneChapters,
    dampen::dampen_loop::SceneSizeList,
    math::{self, FrameScore, ScoreList},
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Scene {
    #[serde(skip_serializing, skip_deserializing)]
    pub index: u32,
    #[serde(skip_serializing, skip_deserializing)]
    pub crf: f64,
    pub start_frame: u32,
    pub end_frame: u32,
    pub zone_overrides: Option<ZoneOverrides>,
    #[serde(skip_serializing, skip_deserializing)]
    pub frame_scores: Vec<FrameScore>,
    #[serde(skip_serializing, skip_deserializing)]
    pub zoned: bool,
}

impl Scene {
    pub fn update_crf(&mut self, new_crf: f64) {
        self.crf = new_crf;
        if let Some(ref mut overrides) = self.zone_overrides {
            overrides.update_crf(new_crf);
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ZoneOverrides {
    pub encoder: Option<String>,
    pub passes: Option<u8>,
    pub video_params: Option<Vec<String>>,
    pub photon_noise: Option<u32>,
    pub photon_noise_height: Option<u32>,
    pub photon_noise_width: Option<u32>,
    pub chroma_noise: bool,
    pub extra_splits_len: Option<u32>,
    pub min_scene_len: Option<u32>,
}

impl ZoneOverrides {
    pub fn from_params(av1an_params: &str, encoder_params: &str, crf: f64) -> ZoneOverrides {
        let mut encoder = None;
        let mut passes = None;
        let mut photon_noise = None;
        let mut min_scene_len: Option<u32> = None;
        let mut extra_splits_len: Option<u32> = None;
        let mut photon_noise_width: Option<u32> = None;
        let mut photon_noise_height: Option<u32> = None;
        let mut chroma_noise = false;

        let mut av1an_tokens = av1an_params.split_whitespace().peekable();
        while let Some(token) = av1an_tokens.next() {
            match token {
                "--encoder" => {
                    if let Some(value) = av1an_tokens.next() {
                        let value = if value == "svt-av1" { "svt_av1" } else { value };
                        encoder = Some(value.to_string());
                    }
                }
                "--passes" => {
                    if let Some(value) = av1an_tokens.next() {
                        passes = value.parse().ok();
                    }
                }
                "--photon-noise" => {
                    if let Some(value) = av1an_tokens.next() {
                        photon_noise = value.parse().ok();
                    }
                }
                "--photon-noise-width" => {
                    if let Some(value) = av1an_tokens.next() {
                        photon_noise_width = value.parse().ok();
                    }
                }
                "--photon-noise-height" => {
                    if let Some(value) = av1an_tokens.next() {
                        photon_noise_height = value.parse().ok();
                    }
                }
                "--chroma-noise" => {
                    chroma_noise = true;
                }
                "--min-scene-len" => {
                    if let Some(value) = av1an_tokens.next() {
                        min_scene_len = value.parse().ok();
                    }
                }
                "--extra-split" => {
                    if let Some(value) = av1an_tokens.next() {
                        extra_splits_len = value.parse().ok();
                    }
                }
                _ => {}
            }
        }

        let mut video_params_vec = encoder_params
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        video_params_vec.push("--crf".to_string());
        video_params_vec.push(format!("{crf}"));

        ZoneOverrides {
            encoder,
            passes: passes.or(Some(1)),
            video_params: Some(video_params_vec),
            photon_noise,
            photon_noise_height,
            photon_noise_width,
            chroma_noise,
            extra_splits_len: extra_splits_len.or(Some(0)),
            min_scene_len: min_scene_len.or(Some(0)),
        }
    }

    pub fn update_from_params(&mut self, av1an_params: &str, encoder_params: &str, crf: u8) {
        let mut encoder = None;
        let mut passes = None;
        let mut photon_noise = None;
        let mut min_scene_len: Option<u32> = None;
        let mut extra_splits_len: Option<u32> = None;
        let mut photon_noise_width: Option<u32> = None;
        let mut photon_noise_height: Option<u32> = None;
        let mut chroma_noise = false;

        let mut av1an_tokens = av1an_params.split_whitespace().peekable();
        while let Some(token) = av1an_tokens.next() {
            match token {
                "--encoder" => {
                    if let Some(value) = av1an_tokens.next() {
                        let value = if value == "svt-av1" { "svt_av1" } else { value };
                        encoder = Some(value.to_string());
                    }
                }
                "--passes" => {
                    if let Some(value) = av1an_tokens.next() {
                        passes = value.parse().ok();
                    }
                }
                "--photon-noise" => {
                    if let Some(value) = av1an_tokens.next() {
                        photon_noise = value.parse().ok();
                    }
                }
                "--photon-noise-width" => {
                    if let Some(value) = av1an_tokens.next() {
                        photon_noise_width = value.parse().ok();
                    }
                }
                "--photon-noise-height" => {
                    if let Some(value) = av1an_tokens.next() {
                        photon_noise_height = value.parse().ok();
                    }
                }
                "--chroma-noise" => {
                    chroma_noise = true;
                }
                "--min-scene-len" => {
                    if let Some(value) = av1an_tokens.next() {
                        min_scene_len = value.parse().ok();
                    }
                }
                "--extra-split" => {
                    if let Some(value) = av1an_tokens.next() {
                        extra_splits_len = value.parse().ok();
                    }
                }
                _ => {}
            }
        }

        let mut video_params_vec = encoder_params
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        video_params_vec.push("--crf".to_string());
        video_params_vec.push(format!("{crf}"));

        self.encoder = encoder;
        self.passes = passes.or(Some(1));
        self.video_params = Some(video_params_vec);
        self.photon_noise = photon_noise;
        self.photon_noise_height = photon_noise_height;
        self.photon_noise_width = photon_noise_width;
        self.chroma_noise = chroma_noise;
        self.extra_splits_len = extra_splits_len.or(Some(240));
        self.min_scene_len = min_scene_len.or(Some(24));
    }

    /// Update only the CRF value in `video_params`
    pub fn update_crf(&mut self, crf: f64) {
        if let Some(ref mut params) = self.video_params {
            let mut found = false;

            for i in 0..params.len() {
                if params[i] == "--crf" && i + 1 < params.len() {
                    params[i + 1] = crf.to_string();
                    found = true;
                    break;
                }
            }

            if !found {
                params.push("--crf".to_string());
                params.push(crf.to_string());
            }
        } else {
            self.video_params = Some(vec!["--crf".to_string(), crf.to_string()]);
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SceneList {
    pub frames: u32,
    pub scenes: Vec<Scene>,
    pub split_scenes: Vec<Scene>,
}

impl SceneList {
    pub fn with_middle_frames(&self) -> SceneList {
        let mut scenes = Vec::with_capacity(self.split_scenes.len());

        for scene in &self.split_scenes {
            let middle_frame = if !scene.frame_scores.is_empty() {
                scene.frame_scores[scene.frame_scores.len() / 2].frame
            } else {
                0 // fallback for empty scenes
            };

            scenes.push(Scene {
                start_frame: scene.start_frame, // Keep original
                end_frame: scene.end_frame,     // Keep original
                zone_overrides: scene.zone_overrides.clone(),
                frame_scores: vec![middle_frame.into()],
                crf: scene.crf,
                index: scene.index,
                zoned: scene.zoned,
            });
        }

        SceneList {
            frames: self.frames,
            scenes: scenes.clone(),
            split_scenes: scenes,
        }
    }

    pub fn with_contiguous_frames(&self) -> SceneList {
        let mut scenes = Vec::with_capacity(self.split_scenes.len());
        let mut global_counter = 0;

        for scene in &self.split_scenes {
            let frame_count = scene.frame_scores.len() as u32;

            scenes.push(Scene {
                start_frame: global_counter,
                end_frame: global_counter + frame_count,
                ..scene.clone() // Keep all other fields
            });

            global_counter += frame_count;
        }

        SceneList {
            frames: global_counter,
            scenes: scenes.clone(),
            split_scenes: scenes,
        }
    }

    pub fn with_evenly_spaced_frames(&self, n: u32) -> SceneList {
        if n <= 1 {
            return self.with_middle_frames();
        }

        let mut scenes = Vec::with_capacity(self.split_scenes.len());

        for scene in &self.split_scenes {
            let start = scene.start_frame;
            let end = scene.end_frame.saturating_sub(1); // inclusive
            let total = end.saturating_sub(start);

            let frame_values: Vec<u32> = if n == 0 || total == 0 {
                vec![]
            } else if n == 1 {
                vec![(start + end) / 2]
            } else {
                // Integer-based even spacing that includes both start and end
                (0..n)
                    .map(|i| {
                        start
                            + ((total as u64 * i as u64 + (n - 1) as u64 / 2) / (n - 1) as u64)
                                as u32
                    })
                    .collect()
            };

            scenes.push(Scene {
                start_frame: scene.start_frame,
                end_frame: scene.end_frame,
                zone_overrides: scene.zone_overrides.clone(),
                frame_scores: frame_values.into_iter().map(FrameScore::from).collect(),
                crf: scene.crf,
                index: scene.index,
                zoned: scene.zoned,
            });
        }

        SceneList {
            frames: self.frames, // Preserve original total frame count
            scenes: scenes.clone(),
            split_scenes: scenes,
        }
    }

    pub fn with_center_expanding_frames(&self, n: u32) -> SceneList {
        if n <= 1 {
            return self.with_middle_frames();
        }

        let mut scenes = Vec::with_capacity(self.split_scenes.len());

        for scene in &self.split_scenes {
            let start = scene.start_frame;
            let end = scene.end_frame.saturating_sub(1);
            let total = end.saturating_sub(start);

            let frame_values: Vec<u32> = if n == 0 || total == 0 {
                vec![]
            } else {
                let middle = (start + end) / 2;
                let mut frames: Vec<u32> = (0..n)
                    .map(|i| {
                        if i % 2 == 0 {
                            middle + (i / 2)
                        } else {
                            middle.saturating_sub(i.div_ceil(2))
                        }
                    })
                    .filter(|&frame| frame >= start && frame <= end)
                    .collect();
                frames.sort();
                frames
            };

            scenes.push(Scene {
                start_frame: scene.start_frame, // Keep original
                end_frame: scene.end_frame,     // Keep original
                zone_overrides: scene.zone_overrides.clone(),
                frame_scores: frame_values.into_iter().map(FrameScore::from).collect(),
                crf: scene.crf,
                index: scene.index,
                zoned: scene.zoned,
            });
        }

        SceneList {
            frames: self.frames, // Preserve original count
            scenes: scenes.clone(),
            split_scenes: scenes,
        }
    }

    pub fn with_start_middle_end_frames(&self, n_frames: u32) -> SceneList {
        if n_frames <= 1 {
            return self.with_middle_frames();
        }

        let mut scenes = Vec::with_capacity(self.split_scenes.len());

        for scene in &self.split_scenes {
            let start = scene.start_frame;
            let end = scene.end_frame.saturating_sub(1); // end is inclusive
            let total_frames = end - start + 1;

            let frame_values = if n_frames == 0 || total_frames == 0 {
                vec![]
            } else {
                let base = n_frames / 3;
                let extra = n_frames % 3;

                // Start segment (first `base` frames, clamped to total_frames)
                let start_seg: Vec<_> = (start..start + base).take_while(|&f| f <= end).collect();

                // Middle segment (centered, with extra frames)
                let middle_frames = base + extra;
                let middle_center = (start + end) / 2;
                let middle_start = middle_center.saturating_sub(middle_frames / 2);
                let middle_seg: Vec<_> = (middle_start..=middle_start + middle_frames - 1)
                    .filter(|&f| f >= start && f <= end) // Explicit bounds check
                    .collect();

                // End segment (last `base` frames, clamped to total_frames)
                let end_seg: Vec<_> = (end.saturating_sub(base - 1)..=end)
                    .filter(|&f| f >= start) // Avoid underflow if `base > total_frames`
                    .collect();

                // Combine segments, deduplicate, and truncate
                let mut all_frames = start_seg;
                all_frames.extend(middle_seg);
                all_frames.extend(end_seg);
                all_frames.sort_unstable(); // Not strictly needed, but ensures order
                all_frames.dedup(); // Remove duplicates
                all_frames.truncate(n_frames as usize); // Ensure length ≤ n_frames
                all_frames
            };

            scenes.push(Scene {
                start_frame: scene.start_frame,
                end_frame: scene.end_frame,
                zone_overrides: scene.zone_overrides.clone(),
                frame_scores: frame_values.into_iter().map(FrameScore::from).collect(),
                crf: scene.crf,
                index: scene.index,
                zoned: scene.zoned,
            });
        }

        SceneList {
            frames: self.frames,
            scenes: scenes.clone(),
            split_scenes: scenes,
        }
    }

    pub fn update_preset(&mut self, new_preset: i32) {
        for scene in &mut self.split_scenes {
            if let Some(ref mut overrides) = scene.zone_overrides
                && let Some(ref mut params) = overrides.video_params
            {
                let mut found = false;
                for i in 0..params.len() {
                    if params[i] == "--preset" && i + 1 < params.len() {
                        params[i + 1] = new_preset.to_string();
                        found = true;
                    }
                }
                if !found {
                    params.push("--preset".to_string());
                    params.push(new_preset.to_string());
                }
            }
        }
    }

    pub fn with_zone_overrides(&mut self, av1an_params: &str, encoder_params: &str) {
        for scene in &mut self.split_scenes {
            let zone_overrides =
                ZoneOverrides::from_params(av1an_params, encoder_params, scene.crf);
            scene.zone_overrides = Some(zone_overrides);
        }
    }

    pub fn update_crf(&mut self, new_crf: f64) {
        for scene in &mut self.split_scenes {
            scene.update_crf(new_crf);
        }
    }
    pub fn filter_by_frame_score(
        &mut self,
        target_quality: f64,
        min_target_quality: f64,
        new_crf: f64,
        percentile: u8,
    ) {
        self.split_scenes.retain_mut(|scene| {
            let percentile = math::percentile(&scene.frame_scores, percentile);
            let min_score = math::min_score(&scene.frame_scores);
            if (percentile < target_quality) || (min_score < min_target_quality) {
                scene.update_crf(new_crf);
                true
            } else {
                false
            }
        });

        self.frames = self
            .split_scenes
            .iter()
            .map(|scene| scene.frame_scores.len() as u32)
            .sum();
    }

    pub fn filter_by_zoning(&mut self) {
        self.split_scenes.retain_mut(|scene| !scene.zoned);

        self.frames = self
            .split_scenes
            .iter()
            .map(|scene| scene.frame_scores.len() as u32)
            .sum();
    }

    pub fn calculate_crf_percentages(&self) -> Vec<(f64, f64)> {
        let total_frames = self
            .split_scenes
            .iter()
            .map(|scene| scene.end_frame - scene.start_frame)
            .sum::<u32>() as f64;

        let mut frame_counts = HashMap::new();

        for scene in &self.split_scenes {
            let scene_frames = (scene.end_frame - scene.start_frame) as f64;
            // Use a string representation as the key to avoid floating-point precision issues
            let crf_key = format!("{:.2}", scene.crf);
            *frame_counts.entry(crf_key).or_insert(0.0) += scene_frames;
        }

        let mut percentages: Vec<(f64, f64)> = frame_counts
            .into_iter()
            .map(|(crf_str, frames)| {
                let crf_value = crf_str.parse().unwrap_or(0.0); // Safe to unwrap since we formatted it
                (crf_value, (frames / total_frames) * 100.0)
            })
            .collect();

        // Sort in descending order (high to low)
        percentages.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        percentages
    }

    pub fn print_crf_percentages(&self) {
        println!("CRF Distribution:");
        let percentages = self.calculate_crf_percentages();

        for (crf, pct) in percentages {
            println!("CRF {crf}: {pct:.2}%");
        }
    }

    pub fn print_stats(&self) -> Result<()> {
        let score_list = self.to_score_list();
        let stats = score_list.get_stats()?;
        println!("\n{stats}");
        Ok(())
    }

    pub fn write_crf_data(
        &self,
        crf_data_file: Option<&Path>,
        input: &std::path::Path,
        percentile: Option<u8>,
        with_metrics: bool,
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
            let filename = format!("Video: {video_name}\n");
            output.push_str(&filename);

            // Add CRF percentages
            let percentages = self.calculate_crf_percentages();
            let percentages_line = percentages
                .iter()
                .map(|(crf, pct)| format!("CRF {crf}: {pct:.2}%"))
                .collect::<Vec<String>>()
                .join(", ");
            output.push_str("Distribution: ");
            output.push_str(&percentages_line);
            output.push_str("\n\n");

            output.push_str("[DATA]\n");
            // Add chunk details
            for (i, scene) in self.split_scenes.iter().enumerate() {
                if with_metrics {
                    let percentile_score =
                        math::percentile(&scene.frame_scores, percentile.unwrap());
                    let min = math::min_score(&scene.frame_scores);
                    output.push_str(&format!(
                        "scene: {:4}, crf: {:3}, frame-range: {:6} {:6}, {} percentile: {:6.2}, min: {:6.2}\n",
                        i,
                        scene.crf,
                        scene.start_frame,
                        scene.end_frame,
                        percentile.unwrap(),
                        percentile_score,
                        min,
                    ));
                } else {
                    output.push_str(&format!(
                        "scene: {:4}, crf: {:3}, frame-range: {:6} {:6}\n",
                        i, scene.crf, scene.start_frame, scene.end_frame,
                    ));
                }
            }

            // Write everything at once
            std::fs::write(crf_data_file, &output)?;

            // println!(
            //     "CRF data successfully written to {}",
            //     crf_data_file
            //         .as_os_str()
            //         .to_str()
            //         .ok_or_eyre("Invalid UTF-8")?
            // );
        }

        Ok(())
    }

    pub fn all_frames(&self) -> Vec<u32> {
        let mut frames: Vec<u32> = self
            .split_scenes
            .iter()
            .flat_map(|scene| scene.frame_scores.iter().map(|score| score.frame))
            .collect();

        frames.sort_unstable();
        frames.dedup(); // Optional: remove duplicates

        frames
    }

    pub fn frames_to_string(&self) -> String {
        let frames = self.all_frames();
        frames
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn to_score_list(&self) -> ScoreList {
        let scores = self
            .split_scenes
            .iter()
            .flat_map(|scene| scene.frame_scores.clone())
            .collect();

        ScoreList { scores }
    }

    pub fn assign_indexes(&mut self) {
        for (i, scene) in self.split_scenes.iter_mut().enumerate() {
            scene.index = i as u32;
        }
    }

    /// Updates CRF values based on reference scene list (by index)
    pub fn sync_crf_by_index(&mut self, reference: &SceneList) {
        use std::collections::HashMap;

        let crf_map: HashMap<u32, f64> = reference
            .split_scenes
            .iter()
            .map(|scene| (scene.index, scene.crf))
            .collect();

        for scene in &mut self.split_scenes {
            if let Some(new_crf) = crf_map.get(&scene.index) {
                scene.update_crf(*new_crf);
            }
        }
    }

    /// Updates frame scores based on reference scene list (by index)
    pub fn sync_scores_by_index(&mut self, reference: &SceneList) {
        use std::collections::HashMap;

        let scores_map: HashMap<u32, Vec<FrameScore>> = reference
            .split_scenes
            .iter()
            .map(|scene| (scene.index, scene.frame_scores.clone()))
            .collect();

        for scene in &mut self.split_scenes {
            if let Some(new_scores) = scores_map.get(&scene.index) {
                scene.frame_scores = new_scores.clone();
            }
        }
    }

    /// Prints a summary of all scenes including index, CRF, frame range, and mean score
    pub fn print_updated_data(&self, percentile: u8) {
        for (i, scene) in self.split_scenes.iter().enumerate() {
            let percentile_score = math::percentile(&scene.frame_scores, percentile);
            let min = math::min_score(&scene.frame_scores);
            println!(
                "scene: {:4}, crf: {:3}, frame-range: {:6} {:6}, {} percentile: {:6.2}, min: {:6.2}",
                i, scene.crf, scene.start_frame, scene.end_frame, percentile, percentile_score, min
            );
        }
    }

    pub fn update_scenes(&mut self) {
        self.scenes = self.split_scenes.clone();
    }

    pub fn sync_crf_from_zone_overrides(&mut self) -> Result<(), eyre::Report> {
        for (idx, scene) in self.split_scenes.iter_mut().enumerate() {
            let overrides = scene
                .zone_overrides
                .as_ref()
                .ok_or_else(|| eyre::eyre!("Missing zone_overrides in scene {}", idx))?;

            let params = overrides.video_params.as_ref().ok_or_else(|| {
                eyre::eyre!("Missing video_params in zone_overrides for scene {}", idx)
            })?;

            let crf_str = find_crf_value_in_params(params)
                .ok_or_else(|| eyre::eyre!("Missing --crf in video_params for scene {}", idx))?;

            let crf = crf_str.parse::<f64>().map_err(|_| {
                eyre::eyre!(
                    "Failed to parse '{}' as u8 for CRF in scene {}",
                    crf_str,
                    idx
                )
            })?;

            scene.crf = crf;
        }
        Ok(())
    }

    /// Updates CRF values in scenes based on SceneSizeList
    /// Only updates scenes that aren't marked as ready in SceneSizeList
    pub fn update_crfs_from_sizes(&mut self, scene_sizes: &SceneSizeList) -> eyre::Result<()> {
        for scene in &mut self.split_scenes {
            // Find matching scene in SceneSizeList that isn't ready
            if let Some(size_info) = scene_sizes.scenes.iter().find(|s| s.index == scene.index) {
                scene.update_crf(size_info.new_crf);
            }
        }
        Ok(())
    }

    /// Applies CRF values from ZoneChapters to scenes that fall within chapter ranges
    pub fn apply_zone_chapters(&mut self, zone_chapters: &ZoneChapters) {
        for scene in &mut self.split_scenes {
            // Find matching chapter for this scene
            for zone_chapter in &zone_chapters.chapters {
                // Check if the scene falls within the chapter range
                if scene.start_frame <= zone_chapter.end
                    && scene.end_frame >= zone_chapter.start
                    && !zone_chapter.crf.is_nan()
                {
                    scene.update_crf(zone_chapter.crf);
                    scene.zoned = true;
                    break; // Stop checking other chapters once we find a match
                }
            }
        }
    }

    pub fn parse_scene_file(json_path: &Path) -> Result<SceneList> {
        let json_data = fs::read_to_string(json_path)?;
        let scene_list: SceneList = serde_json::from_str(&json_data)?;
        Ok(scene_list)
    }

    pub fn write_scene_list_to_file<'a>(&self, path: &'a Path) -> Result<&'a Path> {
        let json = serde_json::to_string_pretty(&self)?; // pretty format for readability
        fs::write(path, json)?;
        Ok(path)
    }
}

#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum FramesDistribution {
    Center,
    Evenly,
    StartMiddleEnd,
}

// New struct definition
#[derive(Debug, Clone)]
pub struct FrameSelection {
    pub scene_list: SceneList,
    pub n_frames: u32,
    pub distribution: FramesDistribution,
}

#[derive(Debug)]
pub struct CrfPercentage {
    pub crf: u8,
    pub percentage: f64,
}

#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum SceneDetectionMethod {
    Av1an,
    TransnetV2,
}

/// Helper function to extract the CRF value following `--crf` in a parameter list.
pub fn find_crf_value_in_params(params: &[String]) -> Option<&str> {
    let mut iter = params.iter();
    while let Some(arg) = iter.next() {
        if arg == "--crf" {
            return iter.next().map(|s| s.as_str());
        }
    }
    None
}
