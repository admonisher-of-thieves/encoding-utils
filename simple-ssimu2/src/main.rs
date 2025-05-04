use clap::Parser;
use encoding_utils_lib::{math::print_stats, ssimulacra2::ssimu2};
use eyre::Result;
use std::path::PathBuf;

/// Calculate SSIMULACRA2 metric
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input video file
    #[arg(short, long)]
    input: PathBuf,

    /// Output video file (encoded version)
    #[arg(short, long)]
    output: PathBuf,

    /// JSON file containing scene information
    #[arg(short = 'S', long)]
    scenes: Option<PathBuf>,

    /// Frame step value (process every N-th frame)
    #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..))]
    step: u32,

    /// Enable verbose output
    #[arg(short, long = "no-verbose")]
    no_verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Process the videos
    let score_list = if let Some(scenes_file) = args.scenes {
        // If scenes file provided, use scene-based processing
        let scene_list = encoding_utils_lib::scenes::parse_scene_file(&scenes_file)?;
        encoding_utils_lib::ssimulacra2::ssimu2_scenes(
            &args.input,
            &args.output,
            &scene_list,
            !args.no_verbose,
        )?
    } else {
        // Otherwise use frame-by-frame processing with step
        ssimu2(
            &args.input,
            &args.output,
            args.step as usize,
            !args.no_verbose,
        )?
    };

    print_stats(&score_list)?;

    Ok(())
}
