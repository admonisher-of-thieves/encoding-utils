use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

use crate::dampen::dampen_loop::SceneSizeList;

#[derive(Debug, Serialize, Deserialize)]
// #[serde(rename_all = "snake_case")]
pub enum InputType {
    VapourSynth(VapourSynthInput),
    // Add other input types if needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VapourSynthInput {
    pub path: String,
    #[serde(default)]
    pub vspipe_args: Vec<String>,
    pub script_text: String,
    pub is_proxy: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Command {
    Unix(Vec<u8>),
    // Add other command variants if needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProbingStatistic {
    name: String,
    value: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TargetQuality {
    pub vmaf_res: String,
    pub probe_res: Option<String>,
    pub vmaf_scaler: String,
    pub vmaf_filter: Option<String>,
    pub vmaf_threads: u32,
    pub model: Option<String>,
    pub probing_rate: u32,
    pub probes: u32,
    pub target: Option<f64>,
    pub metric: String,
    pub min_q: u32,
    pub max_q: u32,
    pub interp_method: Option<String>,
    pub encoder: String,
    pub pix_format: String,
    pub temp: String,
    pub workers: u32,
    pub video_params: Option<Vec<String>>,
    pub params_copied: bool,
    #[serde(default)]
    pub vspipe_args: Vec<String>,
    pub probing_vmaf_features: Vec<String>,
    pub probing_statistic: ProbingStatistic,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub temp: String,
    pub index: u32,
    pub input: InputType,
    pub proxy: Option<serde_json::Value>, // Can be made more specific if needed
    pub source_cmd: Vec<HashMap<String, Vec<u8>>>,
    pub proxy_cmd: Option<serde_json::Value>, // Can be made more specific if needed
    pub output_ext: String,
    pub start_frame: u32,
    pub end_frame: u32,
    pub frame_rate: f64,
    pub passes: u32,
    pub video_params: Vec<String>,
    pub encoder: String,
    pub noise_size: Vec<Option<serde_json::Value>>,
    pub target_quality: TargetQuality,
    pub per_shot_target_quality_cq: Option<serde_json::Value>,
    pub ignore_frame_mismatch: bool,
}

impl Chunk {
    /// Returns the preset value from video_params if present, or None if not found
    pub fn get_preset(&self) -> Option<i32> {
        // Find the position of "--preset" parameter
        if let Some(preset_pos) = self.video_params.iter().position(|p| p == "--preset") {
            // Check if there's a value after "--preset"
            if preset_pos + 1 < self.video_params.len() {
                // Try to parse the value as i32
                if let Ok(preset_value) = self.video_params[preset_pos + 1].parse::<i32>() {
                    return Some(preset_value);
                }
            }
        }
        None
    }

    /// Returns the CRF value from video_params if present, or None if not found
    pub fn get_crf(&self) -> Option<f64> {
        // Find the position of "--crf" parameter
        if let Some(crf_pos) = self.video_params.iter().position(|p| p == "--crf") {
            // Check if there's a value after "--crf"
            if crf_pos + 1 < self.video_params.len() {
                // Try to parse the value as i32
                if let Ok(crf_value) = self.video_params[crf_pos + 1].parse::<f64>() {
                    return Some(crf_value);
                }
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct ChunkList {
    pub chunks: Vec<Chunk>,
}

impl ChunkList {
    /// Updates the CRF values in video_params based on the SceneSizeList
    /// Only updates scenes that aren't marked as ready
    pub fn update_crf_from_scene_sizes(&mut self, scene_sizes: &SceneSizeList) -> eyre::Result<()> {
        for chunk in &mut self.chunks {
            // Find matching scene in SceneSizeList that isn't ready
            if let Some(scene) = scene_sizes
                .scenes
                .iter()
                .find(|s| s.index == chunk.index && !s.ready)
            {
                // Find position of "--crf" parameter
                if let Some(crf_pos) = chunk.video_params.iter().position(|p| p == "--crf") {
                    // Update the value after "--crf"
                    if crf_pos + 1 < chunk.video_params.len() {
                        chunk.video_params[crf_pos + 1] = scene.new_crf.to_string();
                    } else {
                        // "--crf" was last parameter, add value
                        chunk.video_params.push(scene.new_crf.to_string());
                    }
                } else {
                    // "--crf" not found, add it with the new value
                    chunk.video_params.push("--crf".to_string());
                    chunk.video_params.push(scene.new_crf.to_string());
                }
            }
        }
        Ok(())
    }

    /// Updates the preset values in video_params based on the SceneSizeList
    /// Only updates scenes that aren't marked as ready
    pub fn update_preset_from_scene_sizes(
        &mut self,
        scene_sizes: &SceneSizeList,
        preset: i32,
    ) -> eyre::Result<()> {
        for chunk in &mut self.chunks {
            // Find matching scene in SceneSizeList that isn't ready
            if scene_sizes
                .scenes
                .iter()
                .any(|s| s.index == chunk.index && !s.ready)
            {
                // Find position of "--preset" parameter
                if let Some(preset_pos) = chunk.video_params.iter().position(|p| p == "--preset") {
                    // Update the value after "--preset"
                    if preset_pos + 1 < chunk.video_params.len() {
                        chunk.video_params[preset_pos + 1] = preset.to_string();
                    } else {
                        // "--preset" was last parameter, add value
                        chunk.video_params.push(preset.to_string());
                    }
                } else {
                    // "--preset" not found, add it with the new value
                    chunk.video_params.push("--preset".to_string());
                    chunk.video_params.push(preset.to_string());
                }
            }
        }
        Ok(())
    }

    /// Restores the original preset values in video_params based on the SceneSizeList
    /// Only updates scenes where both size and CRF were modified
    pub fn restore_original_preset_from_scene_sizes(
        &mut self,
        scene_sizes: &SceneSizeList,
    ) -> eyre::Result<()> {
        for chunk in &mut self.chunks {
            // Find matching scene in SceneSizeList where both size AND CRF were modified
            if let Some(scene) = scene_sizes.scenes.iter().find(|s| {
                s.index == chunk.index
                    && s.new_size != s.original_size
                    && s.new_crf != s.original_crf
            }) {
                // Find position of "--preset" parameter
                if let Some(preset_pos) = chunk.video_params.iter().position(|p| p == "--preset") {
                    // Update the value after "--preset" with the original preset
                    if preset_pos + 1 < chunk.video_params.len() {
                        chunk.video_params[preset_pos + 1] = scene.original_preset.to_string();
                    } else {
                        // "--preset" was last parameter, add original value
                        chunk.video_params.push(scene.original_preset.to_string());
                    }
                } else {
                    // "--preset" not found, add it with the original value
                    chunk.video_params.push("--preset".to_string());
                    chunk.video_params.push(scene.original_preset.to_string());
                }
            }
        }
        Ok(())
    }

    pub fn parse_chunks_file(json_path: &Path) -> Result<ChunkList> {
        let json_data = fs::read_to_string(json_path)?;
        let chunks: Vec<Chunk> = serde_json::from_str(&json_data)?;
        Ok(ChunkList { chunks })
    }

    pub fn write_chunks_to_file<'a>(&self, path: &'a Path) -> Result<&'a Path> {
        let json = serde_json::to_string_pretty(&self.chunks)?; // pretty format for readability
        fs::write(path, json)?;
        Ok(path)
    }
}
