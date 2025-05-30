use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use eyre::{OptionExt, Result};

pub fn get_scene_file<'a>(
    input: &'a Path,
    temp_folder: &'a Path,
    av1an_params: &str,
    importer: &SourcePlugin,
    downscale: bool,
    override_file: bool,
) -> Result<PathBuf> {
    let scenes_path = temp_folder.join("scenes.json");

    if override_file && scenes_path.exists() {
        fs::remove_file(&scenes_path)?;
    }

    let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;
    let binding = scenes_path.clone();
    let scene_str = binding.to_str().ok_or_eyre("Invalid UTF-8 in scene path")?;

    println!("Obtaining scene file:\n");

    let av1an_params: Vec<String> = av1an_params
        .split_whitespace()
        .map(str::to_string)
        .collect();

    let mut args: Vec<String> = Vec::from([
        "-i".to_owned(),
        input_str.to_owned(),
        "--scenes".to_owned(),
        scene_str.to_owned(),
        "--sc-only".to_owned(),
    ]);

    if downscale {
        let dimensions = get_dimensions(input, importer, temp_folder)?;
        let mut height = dimensions.height / 2;
        if height % 2 != 0 {
            height -= 1;
        }
        let height_str = height.to_string();
        args.push("--sc-downscale-height".to_owned());
        args.push(height_str);
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
    chunk::Chunk,
    vapoursynth::{SourcePlugin, get_dimensions},
};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Scene {
    pub start_frame: u32,
    pub end_frame: u32,
    pub zone_overrides: Option<ZoneOverrides>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ZoneOverrides {
    pub encoder: Option<String>,
    pub passes: Option<u8>,
    pub video_params: Option<Vec<String>>,
    pub photon_noise: Option<u32>,
    pub extra_splits_len: Option<u32>,
    pub min_scene_len: Option<u32>,
}

impl ZoneOverrides {
    pub fn from(av1an_params: &str, encoder_params: &str, crf: u8) -> ZoneOverrides {
        let mut encoder = None;
        let mut passes = None;
        let mut photon_noise = None;
        let mut min_scene_len: Option<u32> = None;
        let mut extra_splits_len: Option<u32> = None;

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
        video_params_vec.push(format!("{}", crf));

        ZoneOverrides {
            encoder,
            passes: passes.or(Some(1)),
            video_params: Some(video_params_vec),
            photon_noise,
            extra_splits_len: extra_splits_len.or(Some(240)),
            min_scene_len: min_scene_len.or(Some(24)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SceneList {
    pub scenes: Vec<Scene>,
    pub frames: u32,
}

impl SceneList {
    /// Returns a vector of middle frames for each scene
    pub fn middle_frames(&self) -> Vec<u32> {
        self.scenes
            .iter()
            .map(|scene| (scene.start_frame + scene.end_frame) / 2)
            .collect()
    }

    pub fn get_scene_file_with_crf_list(
        &self,
        av1an_params: &str,
        encoder_params: &str,
        crf_list: &[Chunk],
    ) -> Self {
        let scenes = self
            .scenes
            .iter()
            .zip(crf_list.iter())
            .map(|(scene, crf)| {
                let zone_overrides = ZoneOverrides::from(av1an_params, encoder_params, crf.crf);
                Scene {
                    start_frame: scene.start_frame,
                    end_frame: scene.end_frame,
                    zone_overrides: Some(zone_overrides),
                }
            })
            .collect();

        SceneList {
            scenes,
            frames: self.scenes.len() as u32,
        }
    }

    pub fn as_middle_frames(&self) -> SceneList {
        let scenes = self
            .scenes
            .iter()
            .enumerate()
            .map(|(i, scene)| {
                let updated_zone_overrides = scene.zone_overrides.as_ref().map(|z| ZoneOverrides {
                    encoder: z.encoder.clone(),
                    passes: z.passes,
                    video_params: z.video_params.clone(),
                    photon_noise: z.photon_noise,
                    extra_splits_len: Some(0),
                    min_scene_len: Some(1),
                });

                Scene {
                    start_frame: i as u32,
                    end_frame: (i + 1) as u32,
                    zone_overrides: updated_zone_overrides,
                }
            })
            .collect();

        SceneList {
            scenes,
            frames: self.scenes.len() as u32,
        }
    }

    pub fn update_preset(&mut self, new_preset: i32) {
        for scene in &mut self.scenes {
            if let Some(ref mut overrides) = scene.zone_overrides {
                if let Some(ref mut params) = overrides.video_params {
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
    }
}

pub fn parse_scene_file(json_path: &Path) -> Result<SceneList> {
    let json_data = fs::read_to_string(json_path)?;
    let scene_list: SceneList = serde_json::from_str(&json_data)?;
    Ok(scene_list)
}

pub fn write_scene_list_to_file<'a>(scene_list: &'a SceneList, path: &'a Path) -> Result<&'a Path> {
    let json = serde_json::to_string_pretty(scene_list)?; // pretty format for readability
    fs::write(path, json)?;
    Ok(path)
}
