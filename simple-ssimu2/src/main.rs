use clap::{ArgAction, Parser};
use encoding_utils_lib::{
    math::get_stats,
    ssimulacra2::ssimu2,
    vapoursynth::{ImporterPlugin, Trim},
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
    #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    steps: u32,

    /// Disable verbose output - Print only stats
    #[arg(short, long = "only-stats", action = ArgAction::SetTrue, default_value_t = false)]
    only_stats: bool,

    /// Importer plugin
    #[arg(short, long = "importer-plugin", default_value = "lsmash")]
    importer_plugin: ImporterPlugin,

    /// Path to stats file (if not provided, stats will only be printed)
    #[arg(short, long = "stats-file")]
    stats_file: Option<PathBuf>,

    /// Trim to sync video: format is "first,last,clip"
    /// Example: "6,18,distorted" or "6,18,d"
    #[arg(short, long)]
    trim: Option<Trim>,

    /// Allows you to use a distorted video composed of middle frames. Needs scenes file
    #[arg(short, long = "middle-frames", action = ArgAction::SetTrue, default_value_t = false)]
    middle_frames: bool,

    /// Keep temporary files (disables automatic cleanup)
    #[arg(
        short = 'k', 
        long = "keep-files",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    keep_files: bool,

    /// Temp folder (default: "[Temp]_<input>.json" if no temp folder given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    temp: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let temp_dir = match args.temp {
        Some(temp) => temp, 
        None => { 
            let output_name = format!(
                "[TEMP]_[SSIMU2]_{}",
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
        if args.middle_frames {
            encoding_utils_lib::ssimulacra2::ssimu2_frames_scenes(
                &args.reference,
                &args.distorted,
                &scene_list,
                &args.importer_plugin,
                &temp_dir,
                !args.only_stats,
            )?
        } else {
            encoding_utils_lib::ssimulacra2::ssimu2_scenes(
                &args.reference,
                &args.distorted,
                &scene_list,
                args.importer_plugin,
                args.trim,
                &temp_dir,
                !args.only_stats,
            )?
        }
    } else {
        // Otherwise use frame-by-frame processing with step
        ssimu2(
            &args.reference,
            &args.distorted,
            args.steps as usize,
            args.importer_plugin,
            args.trim,
            &temp_dir,
            !args.only_stats,
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
