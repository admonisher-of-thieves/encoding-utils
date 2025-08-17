use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

use crate::dampen::dampen_loop::SceneSizeList;

#[derive(Debug, Serialize, Deserialize)]
pub struct FrameInfo {
    pub frames: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Done {
    pub frames: u32,
    pub done: HashMap<String, FrameInfo>,
    pub audio_done: bool,
}

impl Done {
    /// Updates the Done struct based on the SceneSizeList, removing entries for scenes that aren't ready
    pub fn update_from_scene_sizes(&mut self, scene_sizes: &SceneSizeList) -> eyre::Result<()> {
        // First collect all ready scene indices as strings with leading zeros
        let ready_scenes: std::collections::HashSet<String> = scene_sizes
            .scenes
            .iter()
            .filter(|s| s.ready)
            .map(|s| format!("{:05}", s.index)) // Format with leading zeros to match JSON keys
            .collect();

        // Retain only the done entries that have corresponding ready scenes
        self.done
            .retain(|scene_name, _| ready_scenes.contains(scene_name));

        Ok(())
    }

    pub fn parse_done_file(json_path: &Path) -> Result<Done> {
        let json_data = fs::read_to_string(json_path)?;
        let done: Done = serde_json::from_str(&json_data)?;
        Ok(done)
    }

    pub fn write_done_to_file<'a>(&self, path: &'a Path) -> Result<&'a Path> {
        let json = serde_json::to_string_pretty(&self)?; // pretty format for readability
        fs::write(path, json)?;
        Ok(path)
    }
}
