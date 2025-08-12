use std::{fs::File, path::Path};

use crate::{
    scenes::{Scene, SceneList},
    transnetv2::extract_frames::VideoConfig,
};
use eyre::Result;
use ndarray::{Array3, Axis, s};
use ort::{session::Session, value::Tensor};
use std::io::Write;

#[derive(Debug)]
pub struct SceneDetector {
    // Predictions
    pub hardcut_predictions: Vec<f32>,
    pub fade_predictions: Vec<f32>,

    // Scene cut detection parameters
    pub threshold: f32,
    pub min_scene_len: usize,
    pub extra_split: usize,

    // Fade detection parameters
    pub fade_threshold_low: f32,
    pub min_fade_len: usize,
    pub merge_gap: usize,
    // pub fade_threshold_high: f32,

    // Windowing parameters
    pub window_size: usize,
    pub stride: usize,
    pub center_start: usize,
    pub center_end: usize,
}

impl Default for SceneDetector {
    fn default() -> Self {
        Self {
            hardcut_predictions: Vec::new(),
            fade_predictions: Vec::new(),
            threshold: 0.4,    // Default for hard cuts
            min_scene_len: 24, // ~1 second at 24fps
            extra_split: 240,  // ~10 seconds at 24fps
            // fade_threshold_high: 1.0,
            fade_threshold_low: 0.05,
            min_fade_len: 5,
            merge_gap: 4,
            window_size: 100,
            stride: 50,
            center_start: 25,
            center_end: 75,
        }
    }
}

impl SceneDetector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_params(
        threshold: f32,
        min_scene_len: usize,
        extra_split: usize,
        fade_threshold_low: f32,
        min_fade_len: usize,
        merge_gap: usize,
        // fade_threshold_high: f32,
    ) -> Self {
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
            fade_threshold_low,
            min_fade_len,
            merge_gap,
            // fade_threshold_high,
            ..Default::default()
        }
    }

    pub fn predictions(
        &mut self,
        mut session: Session,
        video_config: &VideoConfig,
        path_predictions: Option<&Path>,
    ) -> Result<()> {
        let input_name = session.inputs[0].name.clone();
        // Get both output names - assuming index 0 is single_frame_pred and 1 is all_frames_pred
        let output_names = (
            session.outputs[0].name.clone(), // single_frame_pred
            session.outputs[1].name.clone(), // all_frames_pred
        );

        let padded_frames = video_config.process_frames()?;
        let total_frames = video_config.total_frames;

        // Initialize both prediction vectors
        let mut hardcut_predictions: Vec<f32> = Vec::with_capacity(total_frames);
        let mut fade_predictions: Vec<f32> = Vec::with_capacity(total_frames);
        let mut ptr = 0;

        let pb = video_config.create_progress_bar("Inferring scenes");

        while ptr + self.window_size <= padded_frames.shape()[0] {
            // Get a window of shape [1, window_size, H, W, C]
            let window = padded_frames
                .slice(s![ptr..ptr + self.window_size, .., .., ..])
                .insert_axis(Axis(0));

            let input_tensor = Tensor::from_array(window.to_owned())?;
            let outputs = session.run(vec![(&input_name, input_tensor)])?;

            // Process single_frame predictions
            let single_logits = outputs
                .get(&output_names.0)
                .ok_or_else(|| eyre::eyre!("Single frame output not found"))?
                .try_extract_tensor::<f32>()?;
            let single_array =
                Array3::from_shape_vec((1, self.window_size, 1), single_logits.1.to_vec())?;
            let single_center = single_array.slice(s![0, self.center_start..self.center_end, 0]);
            hardcut_predictions.extend(single_center.iter().copied());

            // Process all_frames predictions
            let all_logits = outputs
                .get(&output_names.1)
                .ok_or_else(|| eyre::eyre!("All frames output not found"))?
                .try_extract_tensor::<f32>()?;
            let all_array =
                Array3::from_shape_vec((1, self.window_size, 1), all_logits.1.to_vec())?;
            let all_center = all_array.slice(s![0, self.center_start..self.center_end, 0]);
            fade_predictions.extend(all_center.iter().copied());

            // Progress update
            let frames_done = self.stride.min(total_frames - ptr);
            pb.inc(frames_done as u64);
            ptr += self.stride;
        }

        pb.finish_with_message("Inference complete");
        println!();

        // Truncate predictions to total_frames
        self.hardcut_predictions =
            hardcut_predictions[..total_frames.min(hardcut_predictions.len())].to_vec();
        self.fade_predictions =
            fade_predictions[..total_frames.min(fade_predictions.len())].to_vec();

        if let Some(path) = path_predictions {
            self.save_predictions_to_file(path)?;
        }

        Ok(())
    }

    pub fn save_predictions_to_file(&self, filename: &Path) -> Result<()> {
        let mut file = File::create(filename)?;

        // Ensure both predictions have the same length
        let len = std::cmp::min(self.hardcut_predictions.len(), self.fade_predictions.len());

        // Write CSV header
        writeln!(file, "frame,hardcut,fade")?;

        for i in 0..len {
            writeln!(
                file,
                "{}, {:.6},{:.6}",
                i, self.hardcut_predictions[i], self.fade_predictions[i]
            )?;
        }

        Ok(())
    }

    pub fn get_hardcut_frames(&self, threshold: f32) -> Vec<usize> {
        let mut scene_cut_frames = Vec::new();

        // Always start with frame 0 as the first boundary
        scene_cut_frames.push(0);
        let mut prev_end = 0;

        for (i, &pred) in self.hardcut_predictions.iter().enumerate() {
            if pred > threshold {
                // Record the frame AFTER the detected cut
                let cut_frame = i + 1; // +1 because cut happens BETWEEN frames

                // Only add if it's a new distinct cut point
                if cut_frame > prev_end {
                    scene_cut_frames.push(cut_frame);
                    prev_end = cut_frame;
                }
            }
        }

        // Add final boundary if needed (end of video)
        let video_end = self.hardcut_predictions.len();
        if prev_end < video_end {
            scene_cut_frames.push(video_end);
        }

        scene_cut_frames
    }

    /// Simple threshold-based fade detection (no trend analysis)
    pub fn detect_fade_segments(&self) -> Vec<(usize, usize)> {
        let mut fade_segments = Vec::new();
        let mut inside_fade = false;
        let mut start_idx = 0;

        for (idx, &confidence) in self.fade_predictions.iter().enumerate() {
            let is_fade_frame = confidence > self.fade_threshold_low;

            match (is_fade_frame, inside_fade) {
                // Entering fade region
                (true, false) => {
                    start_idx = idx;
                    inside_fade = true;
                }
                // Exiting fade region
                (false, true) => {
                    let end_idx = idx - 1;
                    inside_fade = false;

                    // Only keep segments that meet minimum length
                    if end_idx - start_idx + 1 >= self.min_fade_len {
                        fade_segments.push((start_idx, end_idx));
                    }
                }
                // Already in/out of fade - no action needed
                _ => continue,
            }
        }

        // Handle fade at end of video
        if inside_fade {
            let end_idx = self.fade_predictions.len() - 1;
            if end_idx - start_idx + 1 >= self.min_fade_len {
                fade_segments.push((start_idx, end_idx));
            }
        }

        self.merge_fade_segments(fade_segments)
    }

    /// Merges nearby segments using the configured merge_gap
    fn merge_fade_segments(&self, mut segments: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        if segments.is_empty() {
            return segments;
        }

        segments.sort_by_key(|&(start, _)| start);
        let mut merged = Vec::with_capacity(segments.len());
        let (mut prev_start, mut prev_end) = segments[0];

        for &(curr_start, curr_end) in &segments[1..] {
            if curr_start.saturating_sub(prev_end) <= self.merge_gap {
                prev_end = curr_end; // Merge segments
            } else {
                merged.push((prev_start, prev_end));
                prev_start = curr_start;
                prev_end = curr_end;
            }
        }
        merged.push((prev_start, prev_end));

        merged
    }

    pub fn remove_scene_cuts_in_fades(
        scene_cuts: &[usize],
        fade_segments: &[(usize, usize)],
    ) -> Vec<usize> {
        scene_cuts
            .iter()
            .filter(|&&cut| {
                !fade_segments.iter().any(|&(start, end)| {
                    // Convert scene cut position to match fade detection
                    let scene_cut_frame = cut.saturating_sub(1);
                    start <= scene_cut_frame && scene_cut_frame <= end
                })
            })
            .copied()
            .collect()
    }

    /// Combines scene cuts and fade boundaries
    pub fn combine_scene_cuts_and_fades(
        scene_cuts: &[usize],
        fade_segments: &[(usize, usize)],
    ) -> Vec<usize> {
        let fade_boundaries: Vec<usize> = fade_segments
            .iter()
            .flat_map(|&(s, e)| vec![s, e + 1])
            .collect();

        let mut combined: Vec<usize> = scene_cuts
            .iter()
            .chain(fade_boundaries.iter())
            .copied()
            .collect();

        combined.sort_unstable();
        combined.dedup();
        combined
    }

    pub fn add_fades_for_long_scenes(
        &self,
        filtered_cuts: &[usize], // From get_hardcut_frames() (includes 0 and end)
        fade_segments: &[(usize, usize)],
    ) -> Vec<usize> {
        let video_length = self.hardcut_predictions.len();

        // Extract only the start of each fade segment (with safety check)
        let mut fade_boundaries: Vec<usize> = fade_segments
            .iter()
            .map(|&(s, _)| s.min(video_length))
            .collect();
        fade_boundaries.sort_unstable();
        fade_boundaries.dedup();

        // Start with the original cuts (including 0 and end)
        let mut final_cuts = filtered_cuts.to_vec();
        final_cuts.sort_unstable();

        // Process each segment
        let mut i = 0;
        while i < final_cuts.len() - 1 {
            let start = final_cuts[i];
            let end = final_cuts[i + 1];

            if end - start > self.extra_split
                && let Some(&best_boundary) = fade_boundaries
                    .iter()
                    .filter(|&&b| b > start && b < end)
                    .min_by_key(|&&b| b.abs_diff(start + (end - start) / 2))
            {
                final_cuts.insert(i + 1, best_boundary);
                continue; // Re-process the new segment
            }
            i += 1;
        }
        final_cuts
    }

    /// Full pipeline for computing scene changes using configured parameters
    pub fn compute_scene_changes(&self) -> (Vec<usize>, Vec<usize>) {
        // Get hard cut frames using the threshold from the struct
        let hardcuts = self.get_hardcut_frames(self.threshold);

        // Detect fade segments using configured parameters
        let fade_segments = self.detect_fade_segments();

        // Filter and combine
        let only_hardcuts = Self::remove_scene_cuts_in_fades(&hardcuts, &fade_segments);

        // Add back fades that would help split long scenes
        let final_cuts = self.add_fades_for_long_scenes(&only_hardcuts, &fade_segments);

        // let combined = Self::combine_scene_cuts_and_fades(&filtered_cuts, &fade_segments);

        // let mut hardcuts = File::create("hardcuts.txt").unwrap();
        // let mut hardcuts_with_fades = File::create("hardcuts_with_fades.txt").unwrap();

        // for num in &filtered_cuts {
        //     writeln!(hardcuts, "{num}").unwrap();
        // }

        // for num in &final_cuts {
        //     writeln!(hardcuts_with_fades, "{num}").unwrap();
        // }

        (only_hardcuts, final_cuts)
    }

    #[allow(clippy::type_complexity)]
    pub fn predictions_with_fades_to_scenes(&self) -> (Vec<(usize, usize)>, Vec<(usize, usize)>) {
        let (only_hardcuts, final_cuts) = self.compute_scene_changes();

        // Helper function to convert cuts to scenes
        let cuts_to_scenes = |cuts: &[usize], total: usize| -> Vec<(usize, usize)> {
            if cuts.is_empty() {
                return vec![(0, total)];
            }

            let mut scenes = Vec::new();
            let mut prev = 0;

            for &cut in cuts {
                if cut > prev {
                    scenes.push((prev, cut));
                    prev = cut;
                }
            }

            // Add final segment
            if prev < total {
                scenes.push((prev, total));
            }

            scenes
        };

        let total_frames = self.hardcut_predictions.len();
        (
            cuts_to_scenes(&only_hardcuts, total_frames),
            cuts_to_scenes(&final_cuts, total_frames),
        )
    }

    pub fn combine_short_scenes(&self, scenes: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        if scenes.is_empty() {
            return scenes;
        }

        let mut combined = Vec::with_capacity(scenes.len());
        let mut current_start = scenes[0].0;
        let mut current_end = scenes[0].1;

        for &(start, end) in &scenes[1..] {
            let current_length = current_end - current_start;

            if current_length < self.min_scene_len {
                // Always merge short segments forward
                current_end = end;
            } else {
                // Finalize current segment
                combined.push((current_start, current_end));
                current_start = start;
                current_end = end;
            }
        }

        // Handle last accumulated segment
        let last_length = current_end - current_start;
        if last_length < self.min_scene_len && !combined.is_empty() {
            // Merge trailing shorts with previous segment
            let (prev_start, _) = combined.pop().unwrap();
            combined.push((prev_start, current_end));
        } else {
            combined.push((current_start, current_end));
        }

        combined
    }

    pub fn split_large_scenes(&self, scenes: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
        if self.extra_split == 0 {
            return scenes;
        }

        let mut result = Vec::new();

        for (start, end) in scenes {
            let length = end - start; // exclusive end: [start..end)

            if length <= self.extra_split {
                // Scene is small enough, keep as-is
                result.push((start, end));
            } else {
                // Split into two equal parts (rounding down)
                let mid = start + length / 2;

                // Recursively split both halves
                let mut left = self.split_large_scenes(vec![(start, mid)]);
                let mut right = self.split_large_scenes(vec![(mid, end)]);

                // Combine results
                result.append(&mut left);
                result.append(&mut right);
            }
        }

        result
    }

    pub fn predictions_to_scene_list(&self, fade_scenes: bool) -> SceneList {
        let (hardcut_scenes, final_scenes) = self.predictions_with_fades_to_scenes();
        let scenes = if fade_scenes {
            final_scenes
        } else {
            hardcut_scenes.clone()
        };
        let scenes = self.split_large_scenes(scenes);
        let scenes = self.combine_short_scenes(scenes);

        let scenes: Vec<Scene> = scenes
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

        let hardcut_scenes: Vec<Scene> = hardcut_scenes
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
            frames: self.hardcut_predictions.len() as u32,
            scenes: hardcut_scenes,
            split_scenes: scenes,
        }
    }

    pub fn hardcuts_to_scene_list(&self) -> SceneList {
        let (scenes, _) = self.predictions_with_fades_to_scenes();
        // let scenes = self.split_large_scenes(scenes);
        let scenes = self.combine_short_scenes(scenes);

        let scenes: Vec<Scene> = scenes
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
            frames: self.hardcut_predictions.len() as u32,
            scenes: scenes.clone(),
            split_scenes: scenes,
        }
    }
}
