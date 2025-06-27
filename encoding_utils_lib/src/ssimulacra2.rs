use crate::{
    math::{Score, ScoreList},
    scenes::{FramesDistribution, SceneList},
    vapoursynth::{
        SourcePlugin, ToCString, Trim, bestsource_invoke, crop_reference_to_match,
        downscale_resolution, inverse_telecine, lsmash_invoke, select_frames, set_color_metadata,
        synchronize_clips, vszip_metrics,
    },
};

use eyre::{OptionExt, Result, eyre};
use rayon::iter::{ParallelBridge, ParallelIterator};
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

    if let Some(crop_str) = crop {
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
    scene_list: &SceneList,
    n_frames: u32,
    frames_distribution: FramesDistribution,
    importer_plugin: &SourcePlugin,
    temp_dir: &Path,
    verbose: bool,
    color_metadata: &str,
    crop: Option<&str>,
    downscale: bool,
    detelecining: bool,
) -> Result<ScoreList> {
    let core = Core::builder().build();
    let frames = match frames_distribution {
        FramesDistribution::Center => scene_list.center_expanding_frames(n_frames),
        FramesDistribution::Evenly => scene_list.evenly_spaced_frames(n_frames),
    };

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
        detelecining,
        None,
    )?;

    let reference = select_frames(&core, &reference, &frames)?;
    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;

    let mut scores: Vec<Score> = frames
        .iter()
        .enumerate()
        .par_bridge()
        .map(|(i, &frame_num)| {
            let frame = ssimu2
                .get_frame(i as i32)
                .map_err(|e| eyre!(e.to_string_lossy().to_string()))?;
            let props = frame.properties().ok_or_eyre("Props not found")?;
            let score = props.get_float(KeyStr::from_cstr(&"SSIMULACRA2".to_cstring()), 0)?;
            if verbose {
                println!("i: {:6}, Frame: {:6}, Score: {:6.2}", i, frame_num, score);
            }
            Ok(Score {
                frame: frame_num,
                value: score,
            })
        })
        .collect::<Result<_>>()?;

    scores.sort_by_key(|s| s.frame);
    Ok(ScoreList { scores })
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
    detelecining: bool,
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
        detelecining,
        trim,
    )?;

    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;
    let num_frames = ssimu2.info().num_frames;

    let mut scores: Vec<Score> = (1..=num_frames)
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
            Ok(Score {
                frame: n_frame,
                value: score,
            })
        })
        .collect::<Result<_>>()?;

    scores.sort_by_key(|s| s.frame);
    Ok(ScoreList { scores })
}
