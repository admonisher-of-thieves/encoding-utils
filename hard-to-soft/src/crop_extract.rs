use std::{
    fs::{self, create_dir_all, remove_dir_all},
    path::{Path, PathBuf, absolute},
    process::{Command, Stdio},
};

use encoding_utils_lib::vapoursynth::{SourcePlugin, add_extension};

use eyre::{OptionExt, Result};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::sections::{Crop, FrameRange, Section};

#[allow(clippy::too_many_arguments)]
pub fn create_crops_vpy_file<'a>(
    input: &'a Path,
    vpy_file: &'a Path,
    source_plugin: &'a SourcePlugin,
    crop: &Crop,
    frame_range: &FrameRange,
    temp_folder: &'a Path,
) -> Result<&'a Path> {
    let input = absolute(input)?;

    let input_str = input.to_str().ok_or_eyre("Invalid UTF-8 in input path")?;

    let source = match source_plugin {
        SourcePlugin::Lsmash => "core.lsmas.LWLibavSource",
        SourcePlugin::Bestsource => "core.bs.VideoSource",
    };

    let cache_path = temp_folder.join(
        input
            .file_name()
            .ok_or_eyre("Input path has no filename")?
            .to_str()
            .ok_or_eyre("Filename not UTF-8")?,
    );

    let cache_path = match source_plugin {
        SourcePlugin::Lsmash => add_extension("lwi", cache_path),
        SourcePlugin::Bestsource => cache_path,
    };

    let cache_path = absolute(cache_path)?;

    let cache_str = cache_path.to_str().ok_or_eyre("Filename not UTF-8")?;
    let cache = match source_plugin {
        SourcePlugin::Lsmash => format!("cachefile=\"{}\"", cache_str),
        SourcePlugin::Bestsource => format!("cachepath=\"{}\", cachemode=4", cache_str),
    };

    let crop_str = format!(
        "top={}, bottom={}, left={}, right={}",
        crop.top, crop.bottom, crop.left, crop.right
    );

    // Use string formatting to build the vpy script efficiently
    let vpy_script = format!(
        r#"import vapoursynth as vs

core = vs.core

src = {source_plugin}("{input_str}", {cache})

frames = src[{start_frame}:{end_frame}]

yuv = core.resize.Bicubic(frames, format=vs.YUV444P16)

cropped = core.std.Crop(yuv, {crop_str})

cropped.set_output()
"#,
        source_plugin = source,
        input_str = input_str,
        start_frame = frame_range.start.unwrap(),
        end_frame = frame_range.end.unwrap(),
        cache = cache,
        crop_str = crop_str
    );

    fs::write(vpy_file, vpy_script)?;

    Ok(vpy_file)
}

pub fn extract_frames(
    input: &Path,
    source_plugin: SourcePlugin,
    section: &Section,
    total_frames: i32,
    to_override: bool,
    temp_folder: &Path,
) -> Result<()> {
    let mut extract_paths: Vec<ExtractPaths> = vec![];
    let frame_range = section.resolved_frame_range(total_frames);
    let temp_folder = absolute(temp_folder)?;

    for (i, crop) in section.crop.iter().enumerate() {
        let output_name = format!("{}_{}", &section.name, i);
        let output_folder = temp_folder.join(&output_name);
        let output_file = add_extension("jpg", output_folder.join("%d"));
        let vpy_file = add_extension("vpy", temp_folder.join(&output_name));

        if output_folder.exists() & to_override {
            remove_dir_all(&output_folder)?;
        }

        if !output_folder.exists() {
            create_dir_all(&output_folder)?;
        };

        let vpy_path = create_crops_vpy_file(
            input,
            &vpy_file,
            &source_plugin,
            crop,
            &frame_range,
            &temp_folder,
        )?;

        let paths = ExtractPaths {
            vpy: vpy_path.to_owned(),
            ffmpeg_pattern: output_file.to_owned(),
            frames_folder: output_folder.to_owned(),
        };
        extract_paths.push(paths);
    }

    if to_override {
        extract_paths
            .into_par_iter() // parallel!
            .try_for_each(
                |ExtractPaths {
                     vpy,
                     ffmpeg_pattern,
                     frames_folder,
                 }|
                 -> Result<()> {
                    let mut vspipe = Command::new("vspipe")
                        .arg(&vpy)
                        .arg("-")
                        .arg("-c")
                        .arg("y4m")
                        .stdout(Stdio::piped())
                        .stderr(Stdio::inherit())
                        .spawn()?;

                    let mut ffmpeg = Command::new("ffmpeg")
                        .arg("-loglevel")
                        .arg("error")
                        .arg("-i")
                        .arg("-")
                        .arg("-f")
                        .arg("image2")
                        .arg("-qscale:v")
                        .arg("2")
                        .arg("-start_number")
                        .arg(frame_range.start.unwrap().to_string())
                        .arg(&ffmpeg_pattern)
                        .stdin(vspipe.stdout.take().unwrap())
                        .stderr(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .spawn()?;

                    let status_ffmpeg = ffmpeg.wait()?;
                    let status_vspipe = vspipe.wait()?;

                    let output_name = frames_folder
                        .file_name()
                        .ok_or_eyre("Input path has no filename")?
                        .to_str()
                        .ok_or_eyre("Filename not UTF-8")?;

                    if status_ffmpeg.success() && status_vspipe.success() {
                        println!("{} - Frame extraction complete", &output_name)
                    } else {
                        eprintln!("{} - Extraction frames failed.", &output_name);
                    }

                    Ok(())
                },
            )?; // propagate any error from the closure
    }
    Ok(())
}

pub struct ExtractPaths {
    pub vpy: PathBuf,
    pub ffmpeg_pattern: PathBuf,
    pub frames_folder: PathBuf,
}
