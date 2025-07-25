use std::path::Path;

use crate::{
    scenes::SceneList,
    transnetv2::{extract_frames::VideoConfig, inference::SceneDetector, onnx::TransNetSession},
    vapoursynth::{SourcePlugin, add_extension, prepare_clip, resize_format},
};
use eyre::{OptionExt, Result};
use vapoursynth4_rs::{core::Core, node::VideoNode};

#[allow(clippy::too_many_arguments)]
pub fn run_transnetv2(
    video_path: &Path,
    model_path: Option<&Path>,
    use_cpu: bool,
    importer_plugin: SourcePlugin,
    temp_dir: &Path,
    verbose: bool,
    color_metadata: &str,
    crop: Option<&str>,
    downscale: bool,
    detelecine: bool,
    extra_split_seconds: i64,
    extra_split_frames: Option<i64>,
    min_scene_len_sec: i64,
    min_scene_len: Option<i64>,
    threshold: f32,
    fade_threshold_low: f32,
    min_fade_len: i64,
    merge_gap: i64,
    enable_fade_detection: bool,
    save_predictions: bool,
) -> Result<(SceneList, SceneList)> {
    let core = Core::builder().build();

    let src = prepare_clip(
        &core,
        video_path,
        &importer_plugin,
        temp_dir,
        verbose,
        color_metadata,
        crop,
        downscale,
        detelecine,
    )?;

    let src: VideoNode = resize_format(&core, &src, 48, 27, "RGB24")?;
    let info = src.info();
    let total_frames = info.num_frames as usize;
    let extra_split = match extra_split_frames {
        Some(frames) => frames,
        None => {
            ((extra_split_seconds as f64 * info.fps_num as f64) / info.fps_den as f64).ceil() as i64
        }
    };
    let min_scene_len = match min_scene_len {
        Some(frames) => frames,
        None => {
            ((min_scene_len_sec as f64 * info.fps_num as f64) / info.fps_den as f64).ceil() as i64
        }
    };
    let video_config = VideoConfig {
        src,
        total_frames,
        frame_shape: (27, 48, 3).into(),
        batch: 100,
    };

    let transnet_session = TransNetSession::new(model_path, use_cpu)?;
    let mut scene_detection = SceneDetector::with_params(
        threshold,
        min_scene_len.try_into().unwrap(),
        extra_split as usize,
        fade_threshold_low,
        min_fade_len as usize,
        merge_gap as usize,
    );

    let path_predictions = if save_predictions {
        let output_name = format!(
            "[PREDICTIONS]_{}",
            video_path
                .file_stem()
                .ok_or_eyre("No file name")?
                .to_str()
                .ok_or_eyre("Invalid UTF-8 in input path")?
        );
        let path = video_path.with_file_name(output_name);
        Some(add_extension("csv", path))
    } else {
        None
    };

    scene_detection.predictions(
        transnet_session.session,
        &video_config,
        path_predictions.as_deref(),
    )?;
    let scene_list = scene_detection.predictions_to_scene_list(enable_fade_detection);
    let hardcut_scene_list = scene_detection.hardcuts_to_scene_list();

    // println!("{scenes:#?}");

    Ok((scene_list, hardcut_scene_list))
}
