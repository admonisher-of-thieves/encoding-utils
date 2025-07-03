use crate::{
    math::{FrameScore, ScoreList},
    scenes::SceneList,
    vapoursynth::{
        SourcePlugin, ToCString, Trim, bestsource_invoke, crop_reference_to_match,
        downscale_resolution, inverse_telecine, lsmash_invoke, select_frames, set_color_metadata,
        synchronize_clips, vszip_metrics,
    },
};

use eyre::{OptionExt, Result, eyre};
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

    for (scene_index, scene) in scene_list.scenes.iter_mut().enumerate() {
        let updated_scores: Vec<FrameScore> = (scene.start_frame..scene.end_frame)
            .into_par_iter()
            .map(|frame_index| {
                // Get the FrameScore for this position
                let frame_score = scene
                    .frame_scores
                    .get((frame_index - scene.start_frame) as usize)
                    .ok_or_eyre(format!(
                        "Frame index {} out of bounds in scene {}",
                        frame_index, scene_index
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

                Ok(FrameScore {
                    frame: frame_score.frame, // Keep original frame number
                    value,
                })
            })
            .collect::<Result<_>>()?;

        scene.frame_scores = updated_scores;
    }
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

    let (reference, distorted) = prepare_clips(
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

    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;
    let num_frames = ssimu2.info().num_frames;

    let mut scores: Vec<FrameScore> = (1..=num_frames)
        .step_by(step)
        .enumerate()
        .par_bridge()
        .map(|(i, x)| {
            let frame = ssimu2
                .get_frame(x - 1)
                .map_err(|e| eyre!(e.to_string_lossy().to_string()))?;
            let props = frame.properties().ok_or_eyre("Props not found")?;
            let score = props.get_float(KeyStr::from_cstr(&"SSIMULACRA2".to_cstring()), 0)?;
            let n_frame = u32::try_from(i)? * u32::try_from(step)?;
            if verbose {
                println!("Frame: {:6}, Score: {:6.2}", n_frame, score);
            }
            Ok(FrameScore {
                frame: n_frame,
                value: score,
            })
        })
        .collect::<Result<_>>()?;

    scores.sort_by_key(|s| s.frame);
    Ok(ScoreList { scores })
}
