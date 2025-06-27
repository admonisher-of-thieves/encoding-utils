use clap::{ArgAction, Parser};
use encoding_utils_lib::{ math::get_stats, scenes::FramesDistribution, ssimulacra2::ssimu2, vapoursynth::{SourcePlugin, Trim}
};
use eyre::{OptionExt, Result};
use std::{fs::{self, create_dir_all}, path::PathBuf};

/// Calculate SSIMULACRA2 metric - Using vszip
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Reference video file
    #[arg(short, long)]
    reference: PathBuf,

    /// Distorted video file (encoded version)
    #[arg(short, long)]
    distorted: PathBuf,

    /// JSON file containing scene information
    #[arg(short = 'S', long)]
    scenes: Option<PathBuf>,

    /// Frame step value (process every N-th frame)
    #[arg(short = 's', long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    steps: u32,

    /// Enable verbose output - Print all scores
    #[arg(short = 'v', long = "verbose", action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,

    /// Video Source Plugin
    #[arg(short, long = "source-plugin", default_value = "lsmash")]
    source_plugin: SourcePlugin,

    /// Path to stats file (if not provided, stats will only be printed)
    #[arg(short, long = "stats-file")]
    stats_file: Option<PathBuf>,

    /// Trim to sync video: format is "first,last,clip"
    /// Example: "6,18,distorted" or "6,18,d"
    #[arg(short, long)]
    trim: Option<Trim>,

    /// Allows you to use a distorted video composed of n frames. Needs scenes file
    #[arg(short = 'n', long = "middle-frames", default_value_t = 0)]
    n_frames: u32,

    /// How the frames are distributed when encoding
    #[arg(value_enum, short = 'd', long = "frames-distribution", default_value_t = FramesDistribution::Center)]
    frames_distribution: FramesDistribution,

    /// Keep temporary files (disables automatic cleanup)
    #[arg(
        short = 'k', 
        long = "keep-files",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    keep_files: bool,

    /// Color params base on the svt-av1 params
    #[arg(
    long,
        default_value = "--color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio --chroma-sample-position left"
    )]
    color_metadata: String,

    /// Crop (e.g. 1920:816:0:132)
    #[arg(long)]
    crop: Option<String>,

    /// Downscale, using Box Kernel 0.5
    #[arg(
        long, 
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = clap::value_parser!(bool)
    )]
    downscale: bool,

    /// Removes telecine — A process used to convert 24fps film to 29.97fps video using a 3:2 pulldown pattern.
    #[arg(
        long, 
        default_value_t = false,
        action = ArgAction::Set,
        value_parser = clap::value_parser!(bool)
    )]
    detelecine: bool,

    /// Temp folder (default: "[TEMP]_<input>.json" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let temp_dir = match args.temp {
        Some(temp) => temp, 
        None => { 
            let output_name = format!(
                "[TEMP]_{}",
                args.reference
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            args.reference.with_file_name(output_name)
        }
    };

    create_dir_all(&temp_dir)?;

    // Process the videos
    let score_list = if let Some(scenes_file) = args.scenes {
        // If scenes file provided, use scene-based processing
        let scene_list = encoding_utils_lib::scenes::parse_scene_file(&scenes_file)?;
            encoding_utils_lib::ssimulacra2::ssimu2_frames_selected(
                &args.reference,
                &args.distorted,
                &scene_list,
                args.n_frames,
                args.frames_distribution,
                &args.source_plugin,
                &temp_dir,
                args.verbose,
                &args.color_metadata,
                args.crop.as_deref(),
                args.downscale,
                args.detelecine,
            )?
    } else {
        // Otherwise use frame-by-frame processing with step
        ssimu2(
            &args.reference,
            &args.distorted,
            args.steps as usize,
            args.source_plugin,
            args.trim,
            &temp_dir,
            args.verbose,
            &args.color_metadata,
            args.crop.as_deref(),
            args.downscale,
            args.detelecine,
        )?
    };

    let stats = get_stats(&score_list)?;
    let stats_with_filename = format!("[INFO]\nReference: {}\nDistorted: {}\nSteps: {}\n\n{}", args.reference.to_string_lossy(), args.distorted.to_string_lossy(), args.steps, stats);
    if let Some(output_path) = args.stats_file {
        println!("\n{}", stats_with_filename);
        std::fs::write(output_path, stats_with_filename)?;
    } else {
        println!("\n{}", stats_with_filename);
    }

    if !args.keep_files && fs::exists(&temp_dir)? {
        fs::remove_dir_all(&temp_dir)?;
    }

    Ok(())
}
