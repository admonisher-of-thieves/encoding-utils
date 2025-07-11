use encoding_utils_lib::scenes::{Scene, SceneList};
use eyre::Result;
use ndarray::{Array3, Axis, s};
use ort::{session::Session, value::Tensor};

use crate::extract_frames::VideoConfig;

#[derive(Debug)]
pub struct SceneDetector {
    pub predictions: Vec<f32>,
    pub threshold: f32,
    pub window_size: usize,
    pub stride: usize,
    pub center_start: usize,
    pub center_end: usize,
    pub min_scene_len: usize,
    pub extra_split: usize,
}

impl Default for SceneDetector {
    fn default() -> Self {
        Self {
            predictions: Vec::new(),
            threshold: 0.5,
            window_size: 100,
            stride: 50,
            center_start: 25,
            center_end: 75,
            min_scene_len: 24,
            extra_split: 240,
        }
    }
}

impl SceneDetector {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn with_params(threshold: f32, min_scene_len: usize, extra_split: usize) -> Self {
        if extra_split > 0 {
            assert!(
                min_scene_len <= extra_split,
                "min_scene_len ({min_scene_len}) cannot be greater than extra_split ({extra_split})"
            );
        }

        Self {
            threshold,
            min_scene_len,
            extra_split,
            ..Default::default()
        }
    }

    pub fn predictions(&mut self, mut session: Session, video_config: &VideoConfig) -> Result<()> {
        let input_name = session.inputs[0].name.clone();
        let output_name = session.outputs[0].name.clone();

        let padded_frames = video_config.process_frames()?;
        let total_frames = video_config.total_frames;

        let mut predictions: Vec<f32> = Vec::with_capacity(total_frames);
        let mut ptr = 0;

        let pb = video_config.create_progress_bar("Inferring scenes");

        while ptr + self.window_size <= padded_frames.shape()[0] {
            // Get a window of shape [1, window_size, H, W, C]
            let window = padded_frames
                .slice(s![ptr..ptr + self.window_size, .., .., ..])
                .insert_axis(Axis(0));

            let input_tensor = Tensor::from_array(window.to_owned())?;
            let outputs = &session.run(vec![(&input_name, input_tensor)])?;
            let (_, logits_data) = outputs
                .get(&output_name)
                .ok_or_else(|| eyre::eyre!("Output not found"))?
                .try_extract_tensor::<f32>()?;

            // Reshape to [1, window_size, 1]
            let logits_array =
                Array3::from_shape_vec((1, self.window_size, 1), logits_data.to_vec())?;

            // Slice [0, 25:75, 0] for center predictions
            let center_predictions =
                logits_array.slice(s![0, self.center_start..self.center_end, 0]);
            predictions.extend(center_predictions.iter().copied());

            // Progress update
            let frames_done = self.stride.min(total_frames - ptr);
            pb.inc(frames_done as u64);
            ptr += self.stride;
        }

        pb.finish_with_message("Inference complete");

        // Truncate predictions to total_frames (as in np.concatenate(...)[...])
        let truncated_preds = &predictions[..predictions.len().min(total_frames)];

        self.predictions = truncated_preds.to_vec();
        Ok(())
    }

    fn get_initial_scenes(&self, total_frames: usize) -> Vec<(usize, usize)> {
        let scene_changes: Vec<usize> = self
            .predictions
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| if p > self.threshold { Some(i) } else { None })
            .collect();

        let mut scenes = Vec::new();
        let mut prev_start = 0;

        for &change_point in &scene_changes {
            if change_point >= prev_start {
                let scene_length = change_point + 1 - prev_start;

                // If scene is too short, skip this change point (merge with next scene)
                if scene_length < self.min_scene_len {
                    continue;
                }

                scenes.push((prev_start, change_point + 1));
                prev_start = change_point + 1;
            }
        }

        // Handle the last scene
        if total_frames > prev_start {
            let last_scene_length = total_frames - prev_start;

            // If last scene is too short and we have previous scenes, merge with the last scene
            if last_scene_length < self.min_scene_len && !scenes.is_empty() {
                let (last_start, _) = scenes.pop().unwrap();
                scenes.push((last_start, total_frames));
            } else {
                scenes.push((prev_start, total_frames));
            }
        }

        scenes
    }

    pub fn predictions_to_scenes(&self, total_frames: usize) -> Vec<(usize, usize)> {
        // First get the initial scenes respecting min_scene_len
        let mut scenes = self.get_initial_scenes(total_frames);

        // Skip splitting if extra_split is 0
        if self.extra_split == 0 {
            return scenes;
        }

        // Then recursively split scenes that are too long
        let mut i = 0;
        while i < scenes.len() {
            let (start, end) = scenes[i];
            let length = end - start;

            if length > self.extra_split {
                // Split the scene in half
                let split_point = start + length / 2;

                // Replace current scene with two halves
                scenes.remove(i);
                scenes.insert(i, (split_point, end));
                scenes.insert(i, (start, split_point));

                // Don't increment i, we need to check the new scenes
            } else {
                i += 1;
            }
        }

        scenes
    }

    pub fn predictions_to_scene_list(&self, total_frames: usize) -> SceneList {
        let scenes_tuples = self.predictions_to_scenes(total_frames);

        let scenes: Vec<Scene> = scenes_tuples
            .into_iter()
            .enumerate()
            .map(|(idx, (start, end))| Scene {
                index: idx as u32,
                crf: 0, // or any default value
                start_frame: start as u32,
                end_frame: end as u32,
                zone_overrides: None,
                frame_scores: Vec::new(),
            })
            .collect();

        SceneList {
            scenes,
            frames: total_frames as u32,
        }
    }
}
