use clap::{ArgAction, Parser};
use eyre::{OptionExt, Result};
use encoding_utils_lib::main_loop::run_loop;
use std::{fs, path::{absolute, PathBuf}};

/// Scene-based boost that dynamically adjusts CRF.
/// It creates a scene-file with zone overrides
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input video file
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    input: PathBuf,

    /// Output file (default: "[SCENES BOOSTED] <input>.json" if no input given)
    #[arg(short, long, value_parser = clap::value_parser!(PathBuf))]
    output: Option<PathBuf>,

    /// AV1an encoding parameters
    #[arg(
        long,
        default_value = "--verbose --workers 4 --concat mkvmerge --chunk-method lsmash --encoder svt-av1 --split-method av-scenechange --sc-method standard --extra-split 120 --min-scene-len 24"
    )]
    av1an_params: String,

    /// SVT-AV1 encoder parameters
    #[arg(
    long,
        default_value = "--preset 2 --crf 21~36 --tune 2 --keyint -1 --input-depth 10 --color-primaries bt709 --transfer-characteristics bt709 --matrix-coefficients bt709 --color-range studio"
    )]
    encoder_params: String,

    /// Target SSIMULACRA2 score (0-100)
    #[arg(short = 'q', long, default_value_t = 80.0)]
    target_quality: f64,

    /// Velocity tuning preset (-1~13)
    #[arg(short = 'p', long, default_value_t = 4, value_parser = clap::value_parser!(i32).range(-1..=13))]
    velocity_preset: i32,

    /// Frame processing step (1 = every frame)
    #[arg(short = 's', long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..))]
    step: u32,

    /// Keep temporary files (disables automatic cleanup)
    #[arg(
        short = 'k', 
        long = "keep-files",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    keep_files: bool,

    /// Disable overwrite protection (remove the scene file)
    #[arg(
        short = 'F', 
        long = "no-force",
        action = ArgAction::SetTrue,
        default_value_t = false,
    )]
    no_force: bool,

    // Enable verbose output
    #[arg(short, long, action = ArgAction::SetTrue, default_value_t = false)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input_path = absolute(&args.input)?;
    let scene_boosted = match args.output {
        Some(output) => output, 
        None => { 
            let output_name = format!(
                "[SCENES BOOSTED] {}.json",
                args.input
                    .file_stem()
                    .ok_or_eyre("No file name")?
                    .to_str()
                    .ok_or_eyre("Invalid UTF-8 in input path")?
            );
            args.input.with_file_name(output_name)
        }
    };

    if scene_boosted.exists() {
        if !args.no_force {
            fs::remove_file(&scene_boosted)?;
            println!("\nRemoved existing scene file: {}", scene_boosted.display());
        } else {
            eyre::bail!(
                "Scene file {} already exists. Use --force to overwrite",
                scene_boosted.display()
            );
        }
    }

    let temp_folder = args.input.with_file_name(format!(
        "[TEMP] {}",
        args.input
            .file_stem()
            .ok_or_eyre("No file name")?
            .to_str()
            .ok_or_eyre("Invalid UTF-8 in input path")?
    ));
    fs::create_dir_all(&temp_folder)?;

    run_loop(
        &input_path,
        &scene_boosted,
        &args.av1an_params,
        &args.encoder_params,
        args.target_quality,
        args.velocity_preset,
        args.step as usize,
        !args.keep_files,
        args.verbose,
        &temp_folder
    )?;

    Ok(())
}
