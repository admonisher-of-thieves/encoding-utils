use crate::{
    math::{self, FrameScore, ScoreList},
    scenes::SceneList,
    vapoursynth::{
        SourcePlugin, ToCString, Trim, bestsource_invoke, crop_reference_to_match,
        downscale_resolution, inverse_telecine, lsmash_invoke, select_frames, set_color_metadata,
        synchronize_clips, vszip_metrics,
    },
};

use eyre::{Ok, OptionExt, Result, eyre};
use indicatif::{ProgressBar, ProgressStyle};
use quill::*;
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::path::Path;
use vapoursynth4_rs::{
    core::Core,
    frame::Frame,
    map::KeyStr,
    node::{Node, VideoNode},
};

#[allow(clippy::too_many_arguments)]
fn prepare_clips(
    core: &Core,
    reference_path: &Path,
    distorted_path: &Path,
    importer_plugin: &SourcePlugin,
    temp_dir: &Path,
    verbose: bool,
    color_metadata: &str,
    crop: Option<&str>,
    downscale: bool,
    detelecine: bool,
    trim: Option<Trim>,
) -> Result<(VideoNode, VideoNode)> {
    let (mut reference, mut distorted) = match importer_plugin {
        SourcePlugin::Lsmash => (
            lsmash_invoke(core, reference_path, temp_dir)?,
            lsmash_invoke(core, distorted_path, temp_dir)?,
        ),
        SourcePlugin::Bestsource => (
            bestsource_invoke(core, reference_path, temp_dir)?,
            bestsource_invoke(core, distorted_path, temp_dir)?,
        ),
    };

    if verbose {
        println!(
            "Original\nReference: {:?}\nDistorted: {:?}\n",
            reference.info(),
            distorted.info()
        );
    }

    reference = set_color_metadata(core, &reference, color_metadata)?;
    distorted = set_color_metadata(core, &distorted, color_metadata)?;

    if detelecine {
        reference = inverse_telecine(core, &reference)?;
    }

    if downscale {
        reference = downscale_resolution(core, &reference)?;
    }

    if let Some(crop_str) = crop.filter(|s| !s.is_empty()) {
        reference = crop_reference_to_match(core, &reference, crop_str)?;
    }

    if let Some(trim) = trim {
        (reference, distorted) = synchronize_clips(core, &reference, &distorted, &trim)?;
    }

    if verbose {
        println!(
            "Preprocessed\nReference: {:?}\nDistorted: {:?}\n",
            reference.info(),
            distorted.info()
        );
    }

    Ok((reference, distorted))
}

#[allow(clippy::too_many_arguments)]
pub fn ssimu2_frames_selected(
    reference: &Path,
    distorted: &Path,
    scene_list: &mut SceneList,
    // n_frames: u32,
    // frames_distribution: FramesDistribution,
    importer_plugin: &SourcePlugin,
    temp_dir: &Path,
    verbose: bool,
    color_metadata: &str,
    crop: Option<&str>,
    downscale: bool,
    detelecine: bool,
) -> Result<()> {
    let core = Core::builder().build();

    let (reference, distorted) = prepare_clips(
        &core,
        reference,
        distorted,
        importer_plugin,
        temp_dir,
        verbose,
        color_metadata,
        crop,
        downscale,
        detelecine,
        None,
    )?;

    let all_frames: Vec<u32> = scene_list.all_frames();
    let reference = select_frames(&core, &reference, &all_frames)?;
    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;

    // Calculate total frames to process for progress bar
    let total_frames = scene_list.all_frames().len();

    println!("Calculating Metrics");
    let pb = ProgressBar::new(total_frames.try_into().unwrap());
    pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {prefix} {wide_bar} {pos}/{len} {msg}")
            .unwrap(),
    );
    pb.set_prefix("SSIMU2");

    for (scene_index, scene) in scene_list.scenes.iter_mut().enumerate() {
        let updated_scores: Vec<FrameScore> = (scene.start_frame..scene.end_frame)
            .into_par_iter()
            .map(|frame_index| {
                // Get the FrameScore for this position
                let frame_score = scene
                    .frame_scores
                    .get((frame_index - scene.start_frame) as usize)
                    .ok_or_eyre(format!(
                        "Frame index {frame_index} out of bounds in scene {scene_index}"
                    ))?;

                // Get metrics using the frame index (not the frame number)
                let frame = ssimu2
                    .get_frame(frame_index as i32)
                    .map_err(|e| eyre!(e.to_string_lossy().to_string()))?;

                let props = frame
                    .properties()
                    .ok_or_eyre("Frame properties not found")?;
                let value = props.get_float(KeyStr::from_cstr(&"SSIMULACRA2".to_cstring()), 0)?;

                if verbose {
                    println!(
                        "Scene: {:3}, Frame: {:6}, Score: {:6.2}",
                        scene_index, frame_score.frame, value
                    );
                }

                pb.inc(1); // increment progress bar safely from multiple threads

                Ok(FrameScore {
                    frame: frame_score.frame, // Keep original frame number
                    value,
                })
            })
            .collect::<Result<_>>()?;
        scene.frame_scores = updated_scores;
    }

    pb.finish_with_message("DONE");
    println!();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn ssimu2(
    reference: &Path,
    distorted: &Path,
    step: usize,
    importer_plugin: SourcePlugin,
    trim: Option<Trim>,
    temp_dir: &Path,
    verbose: bool,
    color_metadata: &str,
    crop: Option<&str>,
    downscale: bool,
    detelecine: bool,
) -> Result<ScoreList> {
    let core = Core::builder().build();

    let (reference_node, distorted_node) = prepare_clips(
        &core,
        reference,
        distorted,
        &importer_plugin,
        temp_dir,
        verbose,
        color_metadata,
        crop,
        downscale,
        detelecine,
        trim,
    )?;

    let ssimu2 = vszip_metrics(&core, &reference_node, &distorted_node)?;
    let num_frames = ssimu2.info().num_frames;

    let frames_to_process: Vec<u32> = (0..num_frames.try_into().unwrap())
        .step_by(step)
        .collect::<Vec<_>>();
    let pb = ProgressBar::new(frames_to_process.len() as u64);
    pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {prefix} {wide_bar} {pos}/{len} {msg}")
            .unwrap(),
    );
    pb.set_prefix("SSIMU2");

    let mut scores: Vec<FrameScore> = frames_to_process
        .iter()
        .par_bridge()
        .map(|&i| {
            let frame = ssimu2
                .get_frame(i.try_into().unwrap())
                .map_err(|e| eyre!(e.to_string_lossy().to_string()))?;
            let props = frame.properties().ok_or_eyre("Props not found")?;
            let score = props.get_float(KeyStr::from_cstr(&"SSIMULACRA2".to_cstring()), 0)?;

            if verbose {
                println!("Frame: {i:6}, Score: {score:6.2}");
            }

            pb.inc(1); // increment progress bar safely from multiple threads

            Ok(FrameScore {
                frame: i,
                value: score,
            })
        })
        .collect::<Result<_>>()?;

    pb.finish_with_message("DONE");

    scores.sort_by_key(|s| s.frame);

    Ok(ScoreList { scores })
}

pub fn create_plot(
    svg_path: &Path,
    score_list: &ScoreList,
    reference: &Path,
    distorted: &Path,
) -> Result<()> {
    let score_list = &score_list.scores;
    // let frame_scores = score_list.scores;
    let frames: Vec<(f64, f64)> = score_list
        .iter()
        .map(|frame_score| (frame_score.frame as f64, frame_score.value))
        .collect();
    let mean = math::mean(score_list);
    let deviation = math::standard_deviation(score_list);
    let deviation_plus = mean + deviation;
    let deviation_minus = mean - deviation;
    let percentile_list = math::percentiles(score_list)?;
    let five_percentile = &percentile_list.percentiles[1];
    let min = math::min(score_list)?;
    let min_frames: Vec<(f64, f64)> = min
        .scores
        .iter()
        .map(|frame_score| (frame_score.frame as f64, frame_score.value))
        .collect();

    let mean_frames: Vec<(f64, f64)> = score_list
        .iter()
        .map(|frame_score| (frame_score.frame as f64, mean))
        .collect();
    let deviation_plus_frames: Vec<(f64, f64)> = score_list
        .iter()
        .map(|frame_score| (frame_score.frame as f64, deviation_plus))
        .collect();
    let deviation_minus_frames: Vec<(f64, f64)> = score_list
        .iter()
        .map(|frame_score| (frame_score.frame as f64, deviation_minus))
        .collect();
    let five_percentile_frames: Vec<(f64, f64)> = score_list
        .iter()
        .map(|frame_score| (frame_score.frame as f64, five_percentile.score.value))
        .collect();

    let mean_text = format!("Mean: {mean:.2}");
    let deviation_plus_text = format!(
        "Mean + 1 Deviation: {mean:.2} + {deviation:.2} = {:.2}",
        mean + deviation
    );
    let deviation_minus_text = format!(
        "Mean - 1 Deviation: {mean:.2} - {deviation:.2} = {:.2}",
        mean - deviation
    );
    let five_percentile_text = format!("5th Percentile: {:.2}", five_percentile.score.value);
    let min_text = format!(
        "Min: Frame {}, Score {:.2}",
        min_frames[0].0, min_frames[0].1
    );

    let reference_name = reference
        .file_name()
        .ok_or_eyre("Input path has no filename")?
        .to_str()
        .ok_or_eyre("Filename not UTF-8")?;
    let reference_name = format!("Reference: {reference_name}");
    let distorted_name = distorted
        .file_name()
        .ok_or_eyre("Input path has no filename")?
        .to_str()
        .ok_or_eyre("Filename not UTF-8")?;
    let distorted_name = format!("Distorted: {distorted_name}");

    let plot = Plot::builder()
        .dimensions((1280, 720))
        .title("SSIMU2 - Reference vs Distorted")
        .x_label("Frames")
        .y_label("Scores")
        .x_range(Range::Auto)
        .y_range(Range::Manual {
            min: 0.0,
            max: 100.0,
        })
        .legend(Legend::BottomLeftInside)
        .grid(Grid::Dotted)
        .font("Fredoka")
        .data([
            Series::builder()
                .name("SSIMU2 Scores")
                .color("Blue")
                .data(frames)
                .marker(Marker::Circle)
                .marker_size(1.0)
                .line(Line::Solid)
                .interpolation(Interpolation::Linear)
                .line_width(1.0)
                .build(),
            Series::builder()
                .name(&mean_text)
                .color(Color::Green)
                .data(mean_frames)
                .marker(Marker::None)
                .line(Line::Dashed)
                .line_width(2.0)
                .build(),
            Series::builder()
                .name(&deviation_plus_text)
                .color(Color::Orange)
                .data(deviation_plus_frames)
                .marker(Marker::None)
                .line(Line::Dashed)
                .line_width(2.0)
                .build(),
            Series::builder()
                .name(&deviation_minus_text)
                .color(Color::Orange)
                .data(deviation_minus_frames)
                .marker(Marker::None)
                .line(Line::Dashed)
                .line_width(2.0)
                .build(),
            Series::builder()
                .name(&five_percentile_text)
                .color(Color::Magenta)
                .data(five_percentile_frames)
                .marker(Marker::None)
                .line(Line::Dashed)
                .line_width(2.0)
                .build(),
            Series::builder()
                .name(&min_text)
                .color(Color::Red)
                .data(min_frames)
                .marker(Marker::Cross)
                .marker_size(10.0)
                .line(Line::None)
                .build(),
            Series::builder()
                .name(&reference_name)
                .data(vec![])
                .line(Line::None)
                .build(),
            Series::builder()
                .name(&distorted_name)
                .data(vec![])
                .line(Line::None)
                .build(),
        ])
        .build();

    plot.to_svg(svg_path.to_str().ok_or_eyre("Filename not UTF-8")?)?;

    Ok(())
}
