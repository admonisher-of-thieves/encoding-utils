use clap::{ArgAction, Parser};
use encoding_utils_lib::{
    math::get_stats,
    ssimulacra2::ssimu2,
    vapoursynth::{ImporterPlugin, Trim},
};
use eyre::Result;
use std::path::PathBuf;

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
    step: u32,

    /// Disable verbose output - Print only stats
    #[arg(short, long = "only-stats", action = ArgAction::SetTrue, default_value_t = false)]
    only_stats: bool,

    /// Importer plugin
    #[arg(short, long = "importer-plugin", default_value = "lsmash")]
    importer_plugin: ImporterPlugin,

    /// Path to output file (if not provided, stats will only be printed)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Trim to sync video: format is "first,last,clip"
    /// Example: "6,18,distorted" or "6,18,d"
    #[arg(short, long)]
    trim: Option<Trim>,

    /// Allows you to use a distorted video composed of middle frames. Needs scenes file
    #[arg(short, long = "middle-frames", action = ArgAction::SetTrue, default_value_t = false)]
    middle_frames: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Process the videos
    let score_list = if let Some(scenes_file) = args.scenes {
        // If scenes file provided, use scene-based processing
        let scene_list = encoding_utils_lib::scenes::parse_scene_file(&scenes_file)?;
        if args.middle_frames {
            encoding_utils_lib::ssimulacra2::ssimu2_scenes(
                &args.reference,
                &args.distorted,
                &scene_list,
                args.importer_plugin,
                args.trim,
                !args.only_stats,
            )?
        } else {
            encoding_utils_lib::ssimulacra2::ssimu2_frames_scenes(
                &args.reference,
                &args.distorted,
                &scene_list,
                args.importer_plugin,
                !args.only_stats,
            )?
        }
    } else {
        // Otherwise use frame-by-frame processing with step
        ssimu2(
            &args.reference,
            &args.distorted,
            args.step as usize,
            args.importer_plugin,
            args.trim,
            !args.only_stats,
        )?
    };

    let stats = get_stats(&score_list)?;
    if let Some(output_path) = args.output {
        println!("\n{}", stats);
        std::fs::write(output_path, stats)?;
    } else {
        println!("\n{}", stats);
    }

    Ok(())
}
