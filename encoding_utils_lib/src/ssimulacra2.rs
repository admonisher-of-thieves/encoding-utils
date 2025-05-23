use std::path::Path;

use crate::math::{Score, ScoreList};
use crate::scenes::SceneList;
use crate::vapoursynth::{
    SourcePlugin, ToCString, Trim, bestsource_invoke, crop_reference_to_match, lsmash_invoke,
    match_distorted_resolution, resize_bicubic, select_frames, synchronize_clips, vszip_metrics,
};
use eyre::{OptionExt, Result, eyre};
use rayon::prelude::*;
use vapoursynth4_rs::{api::Api, core::Core, frame::Frame, map::KeyStr, node::Node};

pub fn ssimu2_scenes(
    reference: &Path,
    distorted: &Path,
    scene_list: &SceneList,
    importer_plugin: SourcePlugin,
    trim: Option<Trim>,
    temp_dir: &Path,
    verbose: bool,
) -> Result<ScoreList> {
    let api = Api::default();
    let core = Core::builder().api(api).build();

    // Load reference and distorted
    let (mut reference, mut distorted) = match importer_plugin {
        SourcePlugin::Lsmash => (
            lsmash_invoke(&core, reference, temp_dir)?,
            lsmash_invoke(&core, distorted, temp_dir)?,
        ),
        SourcePlugin::Bestsource => (
            bestsource_invoke(&core, reference, temp_dir)?,
            bestsource_invoke(&core, distorted, temp_dir)?,
        ),
    };

    if verbose {
        println!("Original\n");
        println!("Reference: {:?}\n", reference.info());
        println!("Distorted: {:?}\n", distorted.info());
    }

    reference = resize_bicubic(&core, &reference)?;
    distorted = resize_bicubic(&core, &distorted)?;

    // Match resolutions
    reference = match_distorted_resolution(&core, &reference, &distorted)?;

    // Apply cropping if needed
    reference = crop_reference_to_match(&core, &reference, &distorted)?;

    // Apply offset to clips
    if let Some(trim) = trim {
        (reference, distorted) = synchronize_clips(&core, &reference, &distorted, &trim)?;
    }

    let middle_frames = scene_list.middle_frames();

    if verbose {
        println!("Ready to compare\n");
        println!("Reference: {:?}\n", reference.info());
        println!("Distorted: {:?}\n", distorted.info());
    }

    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;

    if verbose {
        println!();
        println!("\nObtaining SSIMU2 Scores\n");
    }
    let mut scores: Vec<Score> = middle_frames
        .par_iter()
        .map(|&x| {
            let frame = ssimu2
                .get_frame(i32::try_from(x)?)
                .map_err(|e| eyre!(e.to_string_lossy().to_string()))?;
            let props = frame.properties().ok_or_eyre("Props not found")?;
            let score = props.get_float(KeyStr::from_cstr(&"SSIMULACRA2".to_cstring()), 0)?;
            if verbose {
                println!("Frame: {:6}, Score: {:6.2}", x, score);
            }
            Ok(Score {
                frame: x,
                value: score,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    scores.sort_by_key(|s| s.frame);

    Ok(ScoreList { scores })
}

pub fn ssimu2_frames_scenes(
    reference: &Path,
    distorted: &Path,
    scene_list: &SceneList,
    importer_plugin: &SourcePlugin,
    temp_dir: &Path,
    verbose: bool,
) -> Result<ScoreList> {
    let api = Api::default();
    let core = Core::builder().api(api).build();

    let middle_frames = scene_list.middle_frames();

    // Load reference and distorted
    let (mut reference, mut distorted) = match importer_plugin {
        SourcePlugin::Lsmash => (
            lsmash_invoke(&core, reference, temp_dir)?,
            lsmash_invoke(&core, distorted, temp_dir)?,
        ),
        SourcePlugin::Bestsource => (
            bestsource_invoke(&core, reference, temp_dir)?,
            bestsource_invoke(&core, distorted, temp_dir)?,
        ),
    };

    if verbose {
        println!("Original\n");
        println!("Reference: {:?}\n", reference.info());
        println!("Distorted: {:?}\n", distorted.info());
    }

    reference = resize_bicubic(&core, &reference)?;
    distorted = resize_bicubic(&core, &distorted)?;

    // Match resolutions
    reference = match_distorted_resolution(&core, &reference, &distorted)?;

    // Apply cropping if needed
    reference = crop_reference_to_match(&core, &reference, &distorted)?;

    let reference = select_frames(&core, &reference, &middle_frames)?;

    if verbose {
        println!("Ready to compare\n");
        println!("Reference: {:?}\n", reference.info());
        println!("Distorted: {:?}\n", distorted.info());
    }

    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;

    if verbose {
        println!("\nObtaining SSIMU2 Scores\n");
    }
    let mut scores: Vec<Score> = middle_frames
        .iter()
        .enumerate()
        .par_bridge()
        .map(|(i, &x)| {
            let frame = ssimu2
                .get_frame(i32::try_from(i)?)
                .map_err(|e| eyre!(e.to_string_lossy().to_string()))?;
            let props = frame.properties().ok_or_eyre("Props not found")?;
            let score = props.get_float(KeyStr::from_cstr(&"SSIMULACRA2".to_cstring()), 0)?;
            if verbose {
                println!("i: {:6}, Frame: {:6}, Score: {:6.2}", i, x, score);
            }
            Ok(Score {
                frame: x,
                value: score,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    scores.sort_by_key(|s| s.frame);

    Ok(ScoreList { scores })
    // Ok(())
}

pub fn ssimu2(
    reference: &Path,
    distorted: &Path,
    step: usize,
    importer_plugin: SourcePlugin,
    trim: Option<Trim>,
    temp_dir: &Path,
    verbose: bool,
) -> Result<ScoreList> {
    let api = Api::default();
    let core = Core::builder().api(api).build();

    // Load reference and distorted
    let (mut reference, mut distorted) = match importer_plugin {
        SourcePlugin::Lsmash => (
            lsmash_invoke(&core, reference, temp_dir)?,
            lsmash_invoke(&core, distorted, temp_dir)?,
        ),
        SourcePlugin::Bestsource => (
            bestsource_invoke(&core, reference, temp_dir)?,
            bestsource_invoke(&core, distorted, temp_dir)?,
        ),
    };

    if verbose {
        println!("Original\n");
        println!("Reference: {:?}\n", reference.info());
        println!("Distorted: {:?}\n", distorted.info());
    }

    reference = resize_bicubic(&core, &reference)?;
    distorted = resize_bicubic(&core, &distorted)?;

    // Match resolutions
    reference = match_distorted_resolution(&core, &reference, &distorted)?;

    // Apply cropping if needed
    reference = crop_reference_to_match(&core, &reference, &distorted)?;

    // Apply offset to clips
    if let Some(trim) = trim {
        (reference, distorted) = synchronize_clips(&core, &reference, &distorted, &trim)?;
    }

    if verbose {
        println!("Ready to compare\n");
        println!("Reference: {:?}\n", reference.info());
        println!("Distorted: {:?}\n", distorted.info());
    }

    let ssimu2 = vszip_metrics(&core, &reference, &distorted)?;

    let info = ssimu2.info();
    let num_frames = info.num_frames;

    if verbose {
        println!("\nObtaining SSIMU2 Scores\n");
    }

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
        .collect::<Result<Vec<_>>>()?;

    scores.sort_by_key(|s| s.frame);

    Ok(ScoreList { scores })
}
