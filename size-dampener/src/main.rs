use bytesize::ByteSize;
use clap::{ArgAction, Parser};
use encoding_utils_lib::{crf::crf_parser, dampen::dampen_loop::dampen_loop};
use eyre::{OptionExt, Result};

use std::{
    fs,
    path::{PathBuf, absolute},
    str::FromStr,
};

/// Scene Dampener that dynamically adjusts CRF.
/// Re-encode av1an scenes until they are below a size threshold
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input video file, you can also pass a .vpy script
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    input: PathBuf,

    /// Output video file
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    output: PathBuf,

    /// Scene file (default: "[BOOST]_<input>.json" if no scene given)
    #[arg(long = "scene-file-input", value_parser = clap::value_parser!(PathBuf))]
    scene_file_input: Option<PathBuf>,

    /// Scene file (default: "[DAMPEN]_<input>.json" if no scene given)
    #[arg(long = "scene-file-output", value_parser = clap::value_parser!(PathBuf))]
    scene_file_output: Option<PathBuf>,

    /// Temp folder (default: "[Temp]_<input>" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,

    /// AV1an encoding parameters
    #[arg(
        long = "av1an-params",
        default_value = "--verbose --workers 1 --concat mkvmerge --chunk-method bestsource --chunk-order sequential --encoder svt-av1 --no-defaults --split-method none --extra-split-sec 0 --min-scene-len 0 "
    )]
    av1an_params: String,

    /// Target size in MiB.
    #[arg(short = 's', long, default_value = "2.5 MiB")]
    size_threshold: String,

    /// Target CRF value(s) (70-1). Can be:
    /// - Single value (35)
    /// - Comma-separated list (35,27,21)
    /// - Range (36..21)
    /// - Stepped range (36..21:3)
    #[arg(short = 'c', long, default_value = "35,30,27,24,21,18")]
    crf: String,

    // Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    /// Path to save the updated crf data
    #[arg(long = "crf-data-file")]
    crf_data_file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let crf_values = crf_parser(&args.crf)?;
    let input_path = absolute(&args.input)?;
    let scene_boosted = match args.scene_file_input {
        Some(output) => output,
        None => {
            let output_name = format!(
                "[BOOST]_{}.json",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            args.input.with_file_name(output_name)
        }
    };

    let scene_dampened = match args.scene_file_output {
        Some(output) => output,
        None => {
            let output_name = format!(
                "[BOOST+DAMPEN]_{}.json",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            args.input.with_file_name(output_name)
        }
    };

    let temp_folder = match args.temp {
        Some(temp) => temp,
        None => args.input.with_file_name(format!(
            "[TEMP]_{}",
            args.input
                .file_stem()
                .ok_or_eyre("No file name")?
                .to_str()
                .ok_or_eyre("Invalid UTF-8 in input path")?
        )),
    };

    fs::create_dir_all(&temp_folder)?;

    let size_threshold = ByteSize::from_str(&args.size_threshold).map_err(|e| eyre::eyre!(e))?;
    dampen_loop(
        &input_path,
        &args.output,
        &scene_boosted,
        &scene_dampened,
        &args.av1an_params,
        &crf_values,
        size_threshold,
        args.crf_data_file.as_deref(),
        &temp_folder,
    )?;

    Ok(())
}
